use super::{
    document::{writer::XmlDocumentRoot, PrintTicketDocument},
    DEFAULT_PRINT_TICKET_XML,
};
use crate::{printer::PrinterDevice, utils::wchar};
use scopeguard::defer;
use std::ptr;
use thiserror::Error;
use windows::{
    core::{BSTR, PCWSTR},
    Win32::{
        Graphics::{
            Gdi::{DM_IN_BUFFER, DM_OUT_BUFFER},
            Printing::{
                ClosePrinter, DocumentPropertiesW, OpenPrinterW,
                PrintTicket::{
                    kPTJobScope, kUserDefaultDevmode, PTCloseProvider, PTConvertDevModeToPrintTicket, PTConvertPrintTicketToDevMode, PTOpenProvider,
                    PTReleaseMemory,
                },
                PRINTER_HANDLE,
            },
        },
        UI::{Shell::SHCreateMemStream, WindowsAndMessaging::IDOK},
    },
};

#[derive(Clone, Debug)]
/// Represents a print ticket.
pub struct PrintTicket {
    pub(crate) xml: Vec<u8>,
}

impl Default for PrintTicket {
    fn default() -> Self {
        Self { xml: DEFAULT_PRINT_TICKET_XML.into() }
    }
}

#[derive(Error, Debug)]
/// Represents an error occurred while converting print ticket to dev mode.
pub enum ToDevModeError {
    /// Failed to open print ticket provider.
    #[error("Failed to open print ticket provider")]
    OpenProviderFailed(#[source] windows::core::Error),
    /// Stream not allocated.
    #[error("Stream not allocated")]
    StreamNotAllocated,
    /// Failed to convert print ticket to dev mode.
    #[error("Failed to convert print ticket to dev mode")]
    ConvertPrintTicketToDevModeFailed(String, #[source] windows::core::Error),
    /// Failed to open printer.
    #[error("Failed to open printer")]
    FailedToOpenPrinter(#[source] windows::core::Error),
    /// Failed to correct dev mode via [`DocumentProperties`].
    ///
    /// [`DocumentProperties`]: https://learn.microsoft.com/en-us/windows/win32/printdocs/documentproperties
    #[error("Failed to correct dev mode via DocumentProperties")]
    FailedToCorrectDevMode,
    /// Opt-in experiment could not establish a safe, auditable single-variable job.
    #[error("{0}")]
    AbValidation(String),
}

impl PrintTicket {
    /// Create a new print ticket from the given XML.
    pub fn from_xml(xml: impl Into<Vec<u8>>) -> Self {
        Self { xml: xml.into() }
    }

    /// Into the XML of the print ticket.
    pub fn into_xml(self) -> Vec<u8> {
        self.xml
    }

    /// Get the XML of the print ticket.
    pub fn get_xml(&self) -> &[u8] {
        &self.xml
    }

    /// A/B-only read-only normalization round trip. Never writes queue defaults or starts a job.
    pub fn normalized_for_ab(&self, device: &PrinterDevice) -> Result<(Vec<u8>, PrintTicket), ToDevModeError> {
        let buffer = self.to_dev_mode(device)?;
        if craft_print_diagnostics::devmode(&buffer).is_none() {
            return Err(ToDevModeError::AbValidation("A/B invalid normalized DEVMODE; nothing sent".into()));
        }
        // Native provider calls stay inside this existing FFI wrapper crate. The owned Unicode
        // DEVMODE came from DocumentPropertiesW; cb bounds the header plus private driver data.
        unsafe {
            let provider = PTOpenProvider(PCWSTR(wchar::to_wide_chars(device.os_name()).as_ptr()), 1).map_err(ToDevModeError::OpenProviderFailed)?;
            defer! { let _ = PTCloseProvider(provider); }
            let stream = SHCreateMemStream(None).ok_or(ToDevModeError::StreamNotAllocated)?;
            let size = buffer
                .get(68..70)
                .and_then(|s| s.try_into().ok())
                .map(u16::from_le_bytes)
                .zip(buffer.get(70..72).and_then(|s| s.try_into().ok()).map(u16::from_le_bytes))
                .map(|(size, extra)| u32::from(size) + u32::from(extra))
                .ok_or_else(|| ToDevModeError::AbValidation("A/B DEVMODE header unavailable; nothing sent".into()))?;
            PTConvertDevModeToPrintTicket(provider, size, buffer.as_ptr().cast(), kPTJobScope, &stream)
                .map_err(|_| ToDevModeError::AbValidation("A/B DEVMODE-to-ticket conversion unavailable; nothing sent".into()))?;
            let mut length = 0;
            stream
                .Seek(0, windows::Win32::System::Com::STREAM_SEEK_END, Some(&mut length))
                .map_err(|_| ToDevModeError::AbValidation("A/B normalized ticket unavailable; nothing sent".into()))?;
            if length > 4 * 1024 * 1024 {
                return Err(ToDevModeError::AbValidation("A/B normalized ticket too large; nothing sent".into()));
            }
            // Owned in-memory stream, no external writer; its checked size cannot grow here.
            let xml = crate::utils::stream::read_com_stream(&stream)
                .map_err(|_| ToDevModeError::AbValidation("A/B normalized ticket read failed; nothing sent".into()))?;
            Ok((buffer, PrintTicket::from_xml(xml)))
        }
    }

    /// Convert the print ticket to [`DEVMODE`] data.
    ///
    /// [`DEVMODE`]: https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-devmodew
    pub fn to_dev_mode(&self, device: &PrinterDevice) -> Result<Vec<u8>, ToDevModeError> {
        unsafe {
            let provider =
                PTOpenProvider(PCWSTR(wchar::to_wide_chars(device.os_name()).as_ptr()), 1)
                    .map_err(ToDevModeError::OpenProviderFailed)?;
            defer! {
                let _ = PTCloseProvider(provider);
            }

            let stream = SHCreateMemStream(Some(self.get_xml()))
                .ok_or(ToDevModeError::StreamNotAllocated)?;

            let mut dev_mode_size = 0;
            let mut dev_mode_data = std::ptr::null_mut();
            let mut error_message = BSTR::default();
            PTConvertPrintTicketToDevMode(
                provider,
                &stream,
                kUserDefaultDevmode,
                kPTJobScope,
                ptr::addr_of_mut!(dev_mode_size),
                ptr::addr_of_mut!(dev_mode_data),
                Some(&mut error_message),
            )
            .map_err(|win32_error| {
                ToDevModeError::ConvertPrintTicketToDevModeFailed(
                    error_message.to_string(),
                    win32_error,
                )
            })?;
            defer! {
                let _ = PTReleaseMemory(dev_mode_data as *mut _);
            }

            let printer_name = wchar::to_wide_chars(device.os_name());
            let printer_handle = {
                let mut printer_handle = PRINTER_HANDLE::default();
                OpenPrinterW(
                    PCWSTR(printer_name.as_ptr()),
                    ptr::addr_of_mut!(printer_handle),
                    None,
                )
                .map_err(ToDevModeError::FailedToOpenPrinter)?;
                printer_handle
            };
            defer! {
                let _ = ClosePrinter(printer_handle);
            }

            let mut buffer_size = DocumentPropertiesW(
                None,
                printer_handle,
                PCWSTR(printer_name.as_ptr()),
                None,
                None,
                0,
            );

            // Workaround for buggy printer drivers
            // See also: https://chromium.googlesource.com/chromium/src/+/refs/tags/124.0.6347.1/printing/backend/win_helper.cc#586
            buffer_size = buffer_size * 2 + 8192;

            let mut buffer = vec![0u8; buffer_size as usize];
            if DocumentPropertiesW(
                None,
                printer_handle,
                PCWSTR(printer_name.as_ptr()),
                Some(buffer.as_mut_ptr() as *mut _),
                Some(dev_mode_data as *mut _),
                (DM_IN_BUFFER | DM_OUT_BUFFER).0,
            ) != IDOK.0
            {
                return Err(ToDevModeError::FailedToCorrectDevMode);
            }

            Ok(buffer)
        }
    }
}

impl From<PrintTicketDocument> for PrintTicket {
    fn from(value: PrintTicketDocument) -> Self {
        Self {
            xml: value.to_xml(),
        }
    }
}

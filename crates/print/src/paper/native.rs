//! Adapter over the existing pinned safe wrapper. No COM/engine/driver-setting changes.
use super::{Capabilities, Paper, ReadFailure, default_size, read_catalog};
use winprint::{
    printer::PrinterDevice,
    ticket::{FeatureOptionPack as _, FetchPrintCapabilitiesError, PrintCapabilities, PrintTicket, PrintTicketBuilder, PrintTicketBuilderError},
};
fn begin(phase: &str) {
    log::info!("print.paper event=begin phase={} thread={:?}", phase, std::thread::current().id());
}
fn caps_error(error: FetchPrintCapabilitiesError) -> ReadFailure {
    use FetchPrintCapabilitiesError::{CannotGetPrintCapabilities, OpenProviderFailed, ParseError, ReadStreamFailed, StreamNotAllocated};
    let (phase, kind, hresult) = match error {
        OpenProviderFailed(e) => ("capabilities.open", "win32", Some(e.code().0)),
        StreamNotAllocated => ("capabilities.stream", "allocation", None),
        CannotGetPrintCapabilities(_, e) => ("capabilities.get", "win32", Some(e.code().0)),
        ReadStreamFailed(e) => ("capabilities.read", "win32", Some(e.code().0)),
        ParseError(_) => ("capabilities.parse", "xml", None),
    };
    ReadFailure { phase, kind, hresult }
}
fn ticket_error(error: PrintTicketBuilderError) -> ReadFailure {
    use PrintTicketBuilderError::{DecodePrintTicketFailed, MergePrintTicketsFailed, OpenProviderFailed, StreamNotAllocated};
    let (phase, kind, hresult) = match error {
        OpenProviderFailed(e) => ("defaults.open", "win32", Some(e.code().0)),
        MergePrintTicketsFailed(_, e) => ("defaults.merge", "win32", Some(e.code().0)),
        DecodePrintTicketFailed(e) => ("defaults.read", "win32", Some(e.code().0)),
        StreamNotAllocated => ("defaults.stream", "allocation", None),
    };
    ReadFailure { phase, kind, hresult }
}
pub(super) fn read(printer: &str) -> Result<Capabilities, ReadFailure> {
    begin("devices.enumerate");
    let devices = PrinterDevice::all().map_err(|e| {
        let hresult = std::error::Error::source(&e).and_then(|source| source.downcast_ref::<windows_core::Error>()).map(|e| e.code().0);
        ReadFailure { phase: "devices.enumerate", kind: "win32", hresult }
    })?;
    let device = devices.into_iter().find(|d| d.name() == printer).ok_or(ReadFailure { phase: "device.select", kind: "not_found", hresult: None })?;
    read_catalog(
        || {
            begin("capabilities.fetch");
            let caps = PrintCapabilities::fetch(&device).map_err(caps_error)?;
            let papers = caps
                .page_media_sizes()
                .take(1024)
                .map(|m| Paper {
                    name: m.display_name().unwrap_or("Printer paper").to_string(),
                    size: (f64::from(m.size().width_in_micron()) * 72. / 25400., f64::from(m.size().height_in_micron()) * 72. / 25400.),
                })
                .collect();
            Ok(papers)
        },
        || {
            begin("defaults.open");
            let mut builder = PrintTicketBuilder::new(&device).map_err(ticket_error)?;
            begin("defaults.merge");
            builder.merge(PrintTicket::default()).map_err(ticket_error)?;
            begin("defaults.build");
            let ticket = builder.build().map_err(ticket_error)?;
            begin("defaults.parse");
            default_size(ticket.get_xml()).map(Some).ok_or(ReadFailure { phase: "defaults.parse", kind: "missing_media_dimensions", hresult: None })
        },
    )
}

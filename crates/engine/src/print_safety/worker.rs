//! Same-executable worker protocol. No listening socket, shell, helper installation or retry.

use std::io::{Read as _, Write as _};
use std::path::Path;
use std::process::{Child, Command};
use std::process::{ExitCode, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::print::spool::{Duplex, Job, Scaling};

const FLAG: &str = "--pdfcraft-image-print-worker";
const TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ticket {
    printer: Option<String>,
    copies: u32,
    collate: bool,
    duplex: Option<u8>,
    scaling: u8,
    grayscale: bool,
    title: String,
    options: Vec<(String, String)>,
}

impl From<&Job> for Ticket {
    fn from(job: &Job) -> Self {
        Self {
            printer: job.printer.clone(),
            copies: job.copies,
            collate: job.collate,
            duplex: job.duplex.map(|d| match d {
                Duplex::Off => 0,
                Duplex::LongEdge => 1,
                Duplex::ShortEdge => 2,
            }),
            scaling: match job.scaling {
                Scaling::Exact => 0,
                Scaling::ShrinkToFit => 1,
            },
            grayscale: job.grayscale,
            title: job.title.clone(),
            options: job.options.clone(),
        }
    }
}

impl Ticket {
    fn job(self) -> Result<Job, String> {
        if self.copies == 0 || self.copies > 999 {
            return Err("Invalid image-print copy count".into());
        }
        Ok(Job {
            printer: self.printer,
            copies: self.copies,
            collate: self.collate,
            duplex: match self.duplex {
                None => None,
                Some(0) => Some(Duplex::Off),
                Some(1) => Some(Duplex::LongEdge),
                Some(2) => Some(Duplex::ShortEdge),
                _ => return Err("Invalid image-print duplex setting".into()),
            },
            scaling: match self.scaling {
                0 => Scaling::Exact,
                1 => Scaling::ShrinkToFit,
                _ => return Err("Invalid image-print scaling".into()),
            },
            grayscale: self.grayscale,
            title: self.title,
            options: self.options,
        })
    }
}

fn read_limited(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("Image-print worker file exceeds its size limit".into());
    }
    Ok(bytes)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())
}

/// Drop must reap even on error: don't leave a native print process alive with deleted inputs.
struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait(child: Child, timeout: Duration) -> Result<(), String> {
    let mut running = Running(child);
    let start = Instant::now();
    loop {
        if let Some(status) = running.0.try_wait().map_err(|e| e.to_string())? {
            return if status.success() {
                Ok(())
            } else {
                Err(format!(
                    "Image-print worker stopped ({status}). The document is still open. Check the printer queue before retrying; some sheets may already have printed."
                ))
            };
        }
        if start.elapsed() >= timeout {
            return Err(
                "Image printing timed out and its worker was stopped. Check the printer queue before retrying; some sheets may already have printed."
                    .into(),
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg_attr(not(windows), expect(dead_code, reason = "Windows worker launcher is also type-checked in host unit tests"))]
pub(super) fn submit(pdf: &[u8], job: &Job) -> Result<String, String> {
    if pdf.len() > super::MAX_BYTES {
        return Err("Image-print input exceeds its byte limit".into());
    }
    // Keep these inputs alive until the child exits, then remove the complete private directory.
    let dir = tempfile::Builder::new().prefix("pdfcraft-image-print-").tempdir().map_err(|e| e.to_string())?;
    write_new(&dir.path().join("input.pdf"), pdf)?;
    write_new(&dir.path().join("ticket.json"), &serde_json::to_vec(&Ticket::from(job)).map_err(|e| e.to_string())?)?;
    let mut command = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    command.arg(FLAG).arg(dir.path()).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW; same binary, no shell.
    }
    wait(command.spawn().map_err(|e| format!("Cannot start image-print worker: {e}"))?, TIMEOUT)?;
    let result: Result<String, String> = serde_json::from_slice(&read_limited(&dir.path().join("result.json"), 64 * 1024)?)
        .map_err(|e| format!("Invalid image-print worker response: {e}"))?;
    result.map(|msg| if msg.is_empty() { "600 dpi image print submitted".into() } else { format!("600 dpi image print: {msg}") })
}

pub(super) fn entry() -> Option<ExitCode> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new(FLAG)) {
        return None;
    }
    let Some(directory) = args.next() else { return Some(ExitCode::FAILURE) };
    if args.next().is_some() {
        return Some(ExitCode::FAILURE);
    }
    let directory = Path::new(&directory);
    let result = crate::guard(|| {
        let ticket: Ticket = serde_json::from_slice(&read_limited(&directory.join("ticket.json"), 64 * 1024)?).map_err(|e| e.to_string())?;
        let job = ticket.job()?;
        let pdf = super::image_pdf(std::sync::Arc::new(read_limited(&directory.join("input.pdf"), super::MAX_BYTES)?))?;
        crate::print::spool::submit(&pdf, &job).map_err(|e| e.to_string())
    })
    .unwrap_or_else(Err);
    let saved = serde_json::to_vec(&result).map_err(|e| e.to_string()).and_then(|bytes| write_new(&directory.join("result.json"), &bytes));
    Some(if saved.is_ok() { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_mode_is_opt_in() {
        assert!(entry().is_none());
    }

    #[test]
    fn ticket_roundtrip_preserves_all_driver_settings() {
        for duplex in [None, Some(Duplex::Off), Some(Duplex::LongEdge), Some(Duplex::ShortEdge)] {
            for scaling in [Scaling::Exact, Scaling::ShrinkToFit] {
                let job = Job {
                    printer: Some("한글 프린터".into()),
                    copies: 3,
                    collate: false,
                    duplex,
                    scaling,
                    grayscale: true,
                    title: "test".into(),
                    options: vec![("tray".into(), "2".into())],
                };
                let bytes = serde_json::to_vec(&Ticket::from(&job)).unwrap();
                assert_eq!(serde_json::from_slice::<Ticket>(&bytes).unwrap().job().unwrap(), job);
            }
        }
    }

    #[test]
    fn worker_inputs_are_bounded_and_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("input.pdf");
        write_new(&file, b"12345").unwrap();
        assert!(write_new(&file, b"replacement").is_err());
        assert!(read_limited(&file, 4).is_err());
        assert_eq!(read_limited(&file, 5).unwrap(), b"12345");
        let mut ticket = Ticket::from(&Job::default());
        ticket.duplex = Some(99);
        assert!(ticket.job().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn failed_or_stuck_worker_reports_error_and_is_not_retried() {
        let child = Command::new("sh").args(["-c", "exit 23"]).spawn().unwrap();
        let err = wait(child, Duration::from_secs(2)).unwrap_err();
        assert!(err.contains("still open") && err.contains("queue"));
        let child = Command::new("sh").args(["-c", "ulimit -c 0; kill -SEGV $$"]).spawn().unwrap();
        assert!(wait(child, Duration::from_secs(2)).unwrap_err().contains("still open"));
        let child = Command::new("sleep").arg("10").spawn().unwrap();
        assert!(wait(child, Duration::from_millis(10)).unwrap_err().contains("timed out"));
    }
}

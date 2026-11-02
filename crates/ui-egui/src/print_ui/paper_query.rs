//! Advisory, bounded native reads. A UI deadline cannot cancel a blocked Windows API.
use super::{PaperResult, PaperSource, WakeOnExit};
use std::{
    sync::mpsc::{self, Receiver, TryRecvError},
    time::{Duration, Instant},
};
const TIMEOUT: Duration = Duration::from_secs(10);
const MAX_READS: usize = 2;
struct Read {
    id: u64,
    printer: String,
    started: Instant,
    timed_out: bool,
    rx: Receiver<PaperResult>,
}
#[derive(Default)]
pub(super) struct PaperReads {
    reads: Vec<Read>,
    current: Option<u64>,
    next: u64,
}
impl PaperReads {
    pub fn reset(&mut self) {
        self.current = None;
    }
    pub fn poll(&mut self, printer: Option<&str>, now: Instant) -> Option<PaperResult> {
        let mut arrived = None;
        let current = self.current;
        self.reads.retain_mut(|read| {
            let relevant = current == Some(read.id) && printer == Some(read.printer.as_str()) && !read.timed_out;
            match read.rx.try_recv() {
                Ok(result) => {
                    log::info!(
                        "print.paper event=result request={} relevant={} success={} elapsed_ms={}",
                        read.id,
                        relevant,
                        result.is_ok(),
                        now.saturating_duration_since(read.started).as_millis()
                    );
                    if relevant {
                        arrived = Some(result);
                    }
                    false
                }
                Err(TryRecvError::Disconnected) => {
                    log::warn!("print.paper event=worker_stopped request={} relevant={}", read.id, relevant);
                    if relevant {
                        arrived = Some(Err("Paper query worker stopped".into()));
                    }
                    false
                }
                Err(TryRecvError::Empty) => {
                    if !read.timed_out && now.saturating_duration_since(read.started) >= TIMEOUT {
                        read.timed_out = true;
                        log::warn!(
                            "print.paper event=timeout request={} relevant={} elapsed_ms={}",
                            read.id,
                            relevant,
                            now.saturating_duration_since(read.started).as_millis()
                        );
                        if relevant {
                            arrived = Some(Err("Paper query timed out; backend validation still required".into()));
                        }
                    }
                    true
                }
            }
        });
        if arrived.is_some() {
            self.current = None;
        }
        arrived
    }
    pub fn start(&mut self, printer: String, source: PaperSource, ctx: Option<egui::Context>, now: Instant) -> Result<(), String> {
        // Never let an old result for the same printer satisfy a new request.
        self.current = None;
        if self.reads.len() >= MAX_READS {
            log::warn!("print.paper event=busy active_reads={}", self.reads.len());
            return Err("Paper query capacity busy; backend validation still required".into());
        }
        let id = self.next.checked_add(1).ok_or("Paper query generation limit")?;
        self.next = id;
        let (tx, rx) = mpsc::channel();
        if let Some(ctx) = &ctx {
            ctx.request_repaint_after(TIMEOUT);
        }
        let wake = WakeOnExit(ctx);
        let read_for = printer.clone();
        std::thread::Builder::new()
            .name("printer paper".into())
            .spawn(move || {
                let _wake = wake;
                log::info!("print.paper event=start request={} thread={:?}", id, std::thread::current().id());
                let _ = tx.send(source(&printer));
            })
            .map_err(|_| {
                log::warn!("print.paper event=worker_start_failed request={}", id);
                "Paper query worker could not start"
            })?;
        self.current = Some(id);
        // The name is used for identity checks only, never written to the log.
        self.reads.push(Read { id, printer: read_for, started: now, timed_out: false, rx });
        Ok(())
    }
}
#[cfg(test)]
mod tests;

use super::*;
use std::sync::Arc;
fn result() -> PaperResult {
    Ok(super::super::print::paper::Capabilities {
        papers: vec![super::super::print::paper::Paper { name: "supported".into(), size: (200., 300.) }],
        default: None,
    })
}
fn pending(reads: &mut PaperReads, id: u64, printer: &str, now: Instant) -> mpsc::Sender<PaperResult> {
    let (tx, rx) = mpsc::channel();
    reads.reads.push(Read { id, printer: printer.into(), started: now, timed_out: false, rx });
    reads.next = id;
    reads.current = Some(id);
    tx
}
#[test]
fn deadline_is_terminal_for_ui_and_late_success_never_overrides_it() {
    let now = Instant::now();
    let mut reads = PaperReads::default();
    let tx = pending(&mut reads, 1, "Office", now);
    assert!(reads.poll(Some("Office"), now + TIMEOUT - Duration::from_millis(1)).is_none());
    assert!(reads.poll(Some("Office"), now + TIMEOUT).unwrap().unwrap_err().contains("timed out"));
    assert_eq!(reads.reads.len(), 1, "blocked native work remains accounted for");
    assert!(reads.poll(Some("Office"), now + TIMEOUT).is_none(), "one timeout transition");
    tx.send(result()).unwrap();
    assert!(reads.poll(Some("Office"), now + TIMEOUT).is_none());
    assert!(reads.reads.is_empty());
}
#[test]
fn reopening_the_same_printer_never_accepts_a_previous_request() {
    let now = Instant::now();
    let mut reads = PaperReads::default();
    let first = pending(&mut reads, 1, "Office", now);
    reads.reset();
    let second = pending(&mut reads, 2, "Office", now);
    first.send(result()).unwrap();
    assert!(reads.poll(Some("Office"), now).is_none());
    assert_eq!(reads.current, Some(2));
    second.send(result()).unwrap();
    assert!(reads.poll(Some("Office"), now).unwrap().is_ok());
}
#[test]
fn switching_printers_discards_stale_results_and_worker_failure_is_terminal() {
    let now = Instant::now();
    let mut reads = PaperReads::default();
    let first = pending(&mut reads, 1, "Old", now);
    first.send(result()).unwrap();
    assert!(reads.poll(Some("New"), now).is_none());
    let sender = pending(&mut reads, 2, "New", now);
    drop(sender);
    assert!(reads.poll(Some("New"), now).unwrap().unwrap_err().contains("stopped"));
}
#[test]
fn timed_out_calls_cannot_accumulate_unbounded_native_threads() {
    let now = Instant::now();
    let mut reads = PaperReads::default();
    let first = pending(&mut reads, 1, "Old", now);
    let second = pending(&mut reads, 2, "Other", now);
    reads.poll(Some("Other"), now + TIMEOUT);
    let source: PaperSource = Arc::new(|_| Err("must not launch".into()));
    assert!(reads.start("Current".into(), source, None, now + TIMEOUT).is_err());
    assert_eq!(reads.reads.len(), MAX_READS);
    drop(first);
    drop(second);
    reads.poll(Some("Current"), now + TIMEOUT);
    assert!(reads.reads.is_empty());
}

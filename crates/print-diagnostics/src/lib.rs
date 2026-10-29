//! Opt-in, best-effort observations of existing print values. Never configures a printer.
//! Set PDFCRAFT_PRINT_DIAGNOSTICS to an absolute output file path before starting PdfCraft.
use std::{
    cell::RefCell,
    ffi::OsString,
    fmt::{self, Write as _},
    io::Write as _,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

thread_local! { static CURRENT: RefCell<Option<Context>> = const { RefCell::new(None) }; }
static FILE_LOCK: Mutex<()> = Mutex::new(());
static NEXT_JOB: AtomicUsize = AtomicUsize::new(0);
const MAX_FILE: u64 = 16 * 1024 * 1024;
const MAX_LINE: usize = 16 * 1024;
const MAX_EVENTS: usize = 10_000;

/// A captured observation sink; clones preserve the source correlation and event budget.
#[derive(Clone)]
pub struct Context {
    path: PathBuf,
    input: u64,
    job: String,
    events: Arc<AtomicUsize>,
}
impl Context {
    /// Read-only opt-in. Missing, empty or relative settings disable all diagnostics.
    pub fn for_pdf(bytes: &[u8]) -> Option<Self> {
        Self::from_setting(std::env::var_os("PDFCRAFT_PRINT_DIAGNOSTICS"), bytes)
    }
    /// Same parsing as the environment entry, also usable without mutating process state.
    pub fn from_setting(setting: Option<OsString>, bytes: &[u8]) -> Option<Self> {
        let path = PathBuf::from(setting?);
        if !path.is_absolute() {
            return None;
        }
        // A correlation fingerprint, NOT a document identity/security digest. No PDF bytes/names.
        let input = bytes.iter().fold(0xcbf29ce484222325_u64, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100000001b3));
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
        let job = format!("{}-{nanos}-{}", std::process::id(), NEXT_JOB.fetch_add(1, Ordering::Relaxed));
        Some(Self { path, input, job, events: Arc::new(AtomicUsize::new(0)) })
    }
    /// Transfer the parent job ID through the existing isolated worker protocol.
    pub fn job_id(&self) -> &str {
        &self.job
    }
    /// A worker may inherit a bounded numeric/hyphen ID, never an output path or log text.
    pub fn inherit_job(mut self, id: &str) -> Self {
        if !id.is_empty() && id.len() <= 96 && id.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
            self.job = id.to_owned();
        }
        self
    }
}
/// Capture before spawning another print thread. No process-global job state.
pub fn capture() -> Option<Context> {
    CURRENT.with(|v| v.borrow().clone())
}
/// Restore the previous context even during unwinding; nested/concurrent jobs stay separate.
pub fn with_context<R>(context: Option<Context>, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Context>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CURRENT.with(|v| *v.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(CURRENT.with(|v| v.replace(context)));
    f()
}
/// Whether this thread has an explicitly enabled sink. Avoid extra inspection when disabled.
pub fn enabled() -> bool {
    CURRENT.with(|v| v.borrow().is_some())
}

struct Line(String);
impl fmt::Write for Line {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            if self.0.len().saturating_add(c.len_utf8()) > MAX_LINE - 32 {
                break;
            }
            self.0.push(if c.is_control() { ' ' } else { c });
        }
        Ok(())
    }
}
/// Append bounded metadata. Failures, missing directories and full files never fail printing.
pub fn record(stage: &'static str, values: fmt::Arguments<'_>) {
    let Some(context) = capture() else { return };
    if context.events.fetch_add(1, Ordering::Relaxed) >= MAX_EVENTS {
        return;
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let mut line = Line(String::new());
    let _ = write!(line, "unix_ms={} pid={} input={:016x} job={} stage={} ", now.as_millis(), std::process::id(), context.input, context.job, stage);
    let _ = line.write_fmt(values);
    line.0.push('\n');
    let Ok(_lock) = FILE_LOCK.lock() else { return };
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    if let Ok(mut file) = options.open(&context.path)
        && file.metadata().is_ok_and(|m| m.len().saturating_add(line.0.len() as u64) <= MAX_FILE)
    {
        let _ = file.write_all(line.0.as_bytes());
    }
}

/// Public DEVMODEW fields, interpreted only when their dmFields flags are present.
#[derive(Debug, PartialEq)]
pub struct DevMode {
    /// Raw flags, useful for distinguishing a missing value from zero.
    pub fields: u32,
    /// Positive DPI or negative quality enum; negative values are NOT DPI.
    pub print_quality: Option<i16>,
    /// Flagged vertical resolution, possibly invalid/nonpositive.
    pub y_resolution: Option<i16>,
    /// Flagged driver scaling in percent, not an inferred 100%.
    pub scale_percent: Option<i16>,
    /// Positive public values only; not the physical device DPI.
    pub effective_dpi: Option<i16>,
}
/// Bounds-check a normalized Unicode DEVMODE without reading private driver data.
pub fn devmode(bytes: &[u8]) -> Option<DevMode> {
    let word = |at: usize| -> Option<i16> { Some(i16::from_le_bytes(bytes.get(at..at.checked_add(2)?)?.try_into().ok()?)) };
    let size = u16::from_le_bytes(bytes.get(68..70)?.try_into().ok()?);
    let extra = u16::from_le_bytes(bytes.get(70..72)?.try_into().ok()?);
    if size < 98 || usize::from(size).checked_add(usize::from(extra))? > bytes.len() {
        return None;
    }
    let fields = u32::from_le_bytes(bytes.get(72..76)?.try_into().ok()?);
    let print_quality = if fields & 0x400 != 0 { word(90) } else { None };
    let y_resolution = if fields & 0x2000 != 0 { word(96) } else { None };
    let scale_percent = if fields & 0x10 != 0 { word(84) } else { None };
    let effective_dpi = print_quality.into_iter().chain(y_resolution).filter(|v| *v > 0).max();
    Some(DevMode { fields, print_quality, y_resolution, scale_percent, effective_dpi })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_relative_and_unwritable_do_not_affect_result() {
        assert!(Context::from_setting(None, b"pdf").is_none());
        assert!(Context::from_setting(Some("relative.txt".into()), b"pdf").is_none());
        let d = tempfile::tempdir().unwrap();
        let result = with_context(Context::from_setting(Some(d.path().into()), b"pdf"), || {
            record("test", format_args!("ignored"));
            42
        });
        assert_eq!(result, 42);
        assert!(std::fs::read_dir(d.path()).unwrap().next().is_none());
    }
    #[test]
    fn scopes_threads_and_bounded_unicode_metadata() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("trace.txt");
        let ctx = Context::from_setting(Some(path.clone().into_os_string()), b"pdf");
        with_context(ctx, || {
            record("parent", format_args!("a\nb {}", "한".repeat(MAX_LINE)));
            let captured = capture();
            std::thread::spawn(move || with_context(captured, || record("child", format_args!("ok")))).join().unwrap();
            with_context(None, || record("disabled", format_args!("absent")));
        });
        record("outside", format_args!("absent"));
        let text = std::fs::read_to_string(path).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(!text.contains("disabled"));
        assert!(text.lines().all(|l| l.len() < MAX_LINE));
        let ids: Vec<_> = text.lines().map(|l| l.split_whitespace().find(|s| s.starts_with("input=")).unwrap()).collect();
        assert_eq!(ids[0], ids[1]);
    }
    #[test]
    fn full_output_is_not_overwritten_and_unwinding_restores_scope() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("trace.txt");
        std::fs::File::create(&path).unwrap().set_len(MAX_FILE).unwrap();
        let ctx = Context::from_setting(Some(path.clone().into_os_string()), b"pdf");
        with_context(ctx.clone(), || record("full", format_args!("ignored")));
        assert_eq!(std::fs::metadata(path).unwrap().len(), MAX_FILE);
        let _ = std::panic::catch_unwind(|| with_context(ctx, || panic!("fixture")));
        assert!(!enabled());
    }
    #[test]
    fn normalized_fields_respect_flags_enums_and_truncation() {
        let mut b = vec![0; 220];
        b[68..70].copy_from_slice(&220u16.to_le_bytes());
        b[72..76].copy_from_slice(&0x2410u32.to_le_bytes());
        b[90..92].copy_from_slice(&600i16.to_le_bytes());
        b[96..98].copy_from_slice(&1200i16.to_le_bytes());
        b[84..86].copy_from_slice(&94i16.to_le_bytes());
        let m = devmode(&b).unwrap();
        assert_eq!(m.effective_dpi, Some(1200));
        assert_eq!(m.scale_percent, Some(94));
        b[90..92].copy_from_slice(&(-4i16).to_le_bytes());
        b[72..76].copy_from_slice(&0x400u32.to_le_bytes());
        let m = devmode(&b).unwrap();
        assert_eq!(m.print_quality, Some(-4));
        assert_eq!(m.effective_dpi, None);
        assert_eq!(m.scale_percent, None);
        for n in 0..220 {
            assert!(devmode(&b[..n]).is_none());
        }
        b[70..72].copy_from_slice(&1u16.to_le_bytes());
        assert!(devmode(&b).is_none());
    }
}

#[cfg(test)]
mod correlation_tests {
    use super::*;
    #[test]
    fn separate_attempts_share_input_but_worker_inherits_only_a_safe_job_id() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("trace.txt");
        let a = Context::from_setting(Some(path.clone().into_os_string()), b"same PDF").unwrap();
        let b = Context::from_setting(Some(path.into_os_string()), b"same PDF").unwrap();
        assert_eq!(a.input, b.input);
        assert_ne!(a.job_id(), b.job_id());
        let child = b.clone().inherit_job(a.job_id());
        assert_eq!(child.job_id(), a.job_id());
        assert_eq!(b.clone().inherit_job("bad\nstage=fake").job_id(), b.job_id());
        assert_eq!(b.clone().inherit_job(&"1".repeat(97)).job_id(), b.job_id());
    }
}

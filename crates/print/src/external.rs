//! Optional external viewer handoff. No shell, queue writes, PDF conversion or bundled viewer.
use crate::PrintError;
use std::path::Path;

/// Deliberately interactive: the external dialog owns paper, printer and all output options.
/// PdfCraft's imposition options are not silently applied to an original-structure snapshot.
pub fn arguments(pdf: &Path, profile: &Path) -> Vec<std::ffi::OsString> {
    [
        std::ffi::OsString::from("-appdata"),
        profile.as_os_str().into(),
        "-new-window".into(),
        "-print-dialog".into(),
        "-exit-when-done".into(),
        pdf.as_os_str().into(),
    ]
    .into()
}

#[cfg(any(windows, test))]
fn run_with(exe: &Path, pdf: &[u8], run: impl FnOnce(&Path, &Path) -> Result<(), PrintError>) -> Result<(), PrintError> {
    use std::io::Write;
    if !exe.is_absolute() || !exe.is_file() {
        return Err(PrintError::Spool("Choose an existing absolute path to SumatraPDF.exe.".into()));
    }
    // tempfile creates a private, unpredictable directory; no original path/title in its name.
    let dir = tempfile::Builder::new().prefix("pdfcraft-external-").tempdir().map_err(io_error)?;
    let path = dir.path().join("document.pdf");
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(io_error)?;
    file.write_all(pdf).map_err(io_error)?;
    file.sync_all().map_err(io_error)?;
    drop(file); // Windows needs it closed before the viewer opens it.
    // Only this temporary viewer profile is changed; no installed viewer/queue defaults.
    // Prevent process reuse so the snapshot cannot disappear while another instance reads it.
    std::fs::write(
        dir.path().join("SumatraPDF-settings.txt"),
        b"ReuseInstance = false\nRememberOpenedFiles = false\nRememberStatePerDocument = false\nRestoreSession = false\nCheckForUpdates = false\n",
    )
    .map_err(io_error)?;
    // Keep the directory until this dedicated viewer exits, including after PdfCraft closes.
    run(&path, dir.path())
}
#[cfg(any(windows, test))]
fn io_error(_: std::io::Error) -> PrintError {
    PrintError::Spool("External print snapshot could not be created or written.".into())
}
#[cfg(any(windows, test))]
fn wait_for_viewer(exe: &Path, path: &Path, profile: &Path) -> Result<(), PrintError> {
    let status = std::process::Command::new(exe)
        .args(arguments(path, profile))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|_| PrintError::Spool("SumatraPDF could not be started. Check the selected executable.".into()))?;
    if !status.success() {
        return Err(PrintError::Spool(format!("SumatraPDF exited with an error (code {:?}); printing is not confirmed.", status.code())));
    }
    Ok(())
}

/// Wait on a worker thread, never the UI thread. Success only means viewer exit, not printed.
pub fn open_dialog(exe: &Path, pdf: &[u8]) -> Result<String, PrintError> {
    #[cfg(windows)]
    {
        if !exe.extension().and_then(std::ffi::OsStr::to_str).is_some_and(|v| v.eq_ignore_ascii_case("exe")) {
            return Err(PrintError::Spool("Select SumatraPDF.exe, not a shell script or shortcut.".into()));
        }
        run_with(exe, pdf, |path, profile| wait_for_viewer(exe, path, profile))?;
        Ok("SumatraPDF closed; the job may have been printed or cancelled. Check the printer.".into())
    }
    #[cfg(not(windows))]
    {
        let _ = (exe, pdf);
        Err(PrintError::Spool("SumatraPDF external printing is available on Windows only.".into()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arguments_are_separate_and_never_silent_print_or_reuse() {
        let args = arguments(Path::new("C:/private dir/한글 & label.pdf"), Path::new("C:/private profile"));
        assert_eq!(args.last().unwrap(), "C:/private dir/한글 & label.pdf");
        assert!(args.contains(&"-print-dialog".into()));
        assert!(!args.iter().any(|s| s == "-reuse-instance" || s == "-print-to" || s == "-silent"));
    }
    #[cfg(unix)]
    #[test]
    fn mock_viewer_waits_reads_exact_snapshot_and_reports_failure() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("mock viewer");
        // Contributor-authored mock; never invokes a real viewer or print queue.
        std::fs::write(
            &exe,
            b"#!/bin/sh\n[ \"$1\" = -appdata ] || exit 7\n[ \"$4\" = -print-dialog ] || exit 8\n[ \"$(cat \"$6\")\" = fixture ] || exit 9\nexit 0\n",
        )
        .unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o700)).unwrap();
        run_with(&exe, b"fixture", |path, profile| wait_for_viewer(&exe, path, profile)).unwrap();
        std::fs::write(&exe, b"#!/bin/sh\nexit 5\n").unwrap();
        assert!(run_with(&exe, b"fixture", |path, profile| wait_for_viewer(&exe, path, profile)).unwrap_err().to_string().contains("Some(5)"));
        assert!(wait_for_viewer(&dir.path().join("missing"), Path::new("unused"), dir.path()).is_err());
    }
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn snapshot_is_exact_and_removed_after_success_and_error() {
        let exe = std::env::current_exe().unwrap();
        for fail in [false, true] {
            let mut seen = None;
            let result = run_with(&exe, b"original Type3 /Image stream bytes", |p, _| {
                seen = Some(p.to_owned());
                assert!(std::fs::read_to_string(p.parent().unwrap().join("SumatraPDF-settings.txt")).unwrap().contains("ReuseInstance = false"));
                assert_eq!(std::fs::read(p).unwrap(), b"original Type3 /Image stream bytes");
                if fail { Err(PrintError::Spool("test failure".into())) } else { Ok(()) }
            });
            assert_eq!(result.is_err(), fail);
            assert!(!seen.unwrap().exists());
        }
        assert!(run_with(Path::new("relative.exe"), b"", |_, _| Ok(())).is_err());
    }
}

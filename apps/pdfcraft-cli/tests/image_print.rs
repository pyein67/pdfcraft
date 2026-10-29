//! Real executable tests; no physical printer and no user document is used.
use std::process::Command;
use std::sync::Arc;

use serde_json::json;

fn synthetic_type3() -> Vec<u8> {
    synthetic_type3_glyph("1 0 d0 0 0 1 1 re f")
}

fn synthetic_type3_glyph(glyph: &str) -> Vec<u8> {
    let content = "BT /F1 20 Tf 10 10 Td (A) Tj ET";
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 40 40] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_string(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
        "<< /Type /Font /Subtype /Type3 /FontBBox [0 0 1 1] /FontMatrix [1 0 0 1 0 0] /FirstChar 65 /LastChar 65 /Widths [1] /Encoding << /Differences [65 /a] >> /CharProcs << /a 6 0 R >> >>".to_string(),
        format!("<< /Length {} >>\nstream\n{glyph}\nendstream", glyph.len()),
    ];
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", i + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(format!("trailer << /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes());
    pdf
}

#[test]
fn cli_image_export_removes_nested_type3_only_when_requested() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("source.pdf"), synthetic_type3()).unwrap();
    let script = json!([
        {"tool":"doc_open","args":{"path":"source.pdf"}},
        {"tool":"doc_print","args":{"doc":1,"path":"vector.pdf"}},
        {"tool":"doc_print","args":{"doc":1,"path":"image.pdf","print_as_image":true}},
        {"tool":"doc_info","args":{"doc":1}}
    ]);
    std::fs::write(dir.path().join("steps.json"), script.to_string()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_pdfcraft-cli"))
        .args(["run", "--script"])
        .arg(dir.path().join("steps.json"))
        .arg("--root")
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let vector = Arc::new(std::fs::read(dir.path().join("vector.pdf")).unwrap());
    let image = Arc::new(std::fs::read(dir.path().join("image.pdf")).unwrap());
    assert!(pdfcraft_engine::print_safety::has_type3(vector).unwrap());
    assert!(!pdfcraft_engine::print_safety::has_type3(image).unwrap());
    assert_eq!(std::fs::read(dir.path().join("source.pdf")).unwrap(), synthetic_type3());
}

#[cfg(windows)]
#[test]
fn windows_isolated_image_submission_returns_an_error_instead_of_closing_the_parent() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("source.pdf"), synthetic_type3()).unwrap();
    let script = json!([
        {"tool":"doc_open","args":{"path":"source.pdf"}},
        {"tool":"doc_print","args":{"doc":1,"printer":"PdfCraft_Missing_Queue_7b85e495"}}
    ]);
    std::fs::write(dir.path().join("steps.json"), script.to_string()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_pdfcraft-cli"))
        .args(["run", "--script"])
        .arg(dir.path().join("steps.json"))
        .arg("--root")
        .arg(dir.path())
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1), "{}", String::from_utf8_lossy(&result.stderr));
    assert!(String::from_utf8_lossy(&result.stderr).contains("not available"), "{}", String::from_utf8_lossy(&result.stderr));
}

#[test]
fn cli_compatibility_export_keeps_outlines_at_actual_size_and_stays_confined() {
    let dir = tempfile::tempdir().unwrap();
    let original = synthetic_type3_glyph("1 0 0 0 1 1 d1 0 0 m 1 0 l 1 1 l 0 1 l h f");
    std::fs::write(dir.path().join("source.pdf"), &original).unwrap();
    let script = json!([
        {"tool":"doc_open","args":{"path":"source.pdf"}},
        {"tool":"doc_print","args":{"doc":1,"path":"safe.pdf","layout":"actual","paper":"A4","print_compatibility":true}},
        {"tool":"doc_print","args":{"doc":1,"path":"normal.pdf","layout":"actual","paper":"A4"}}
    ]);
    std::fs::write(dir.path().join("steps.json"), script.to_string()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_pdfcraft-cli"))
        .args(["run", "--script"])
        .arg(dir.path().join("steps.json"))
        .arg("--root")
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("Type 1C"));
    let safe = Arc::new(std::fs::read(dir.path().join("safe.pdf")).unwrap());
    let normal = Arc::new(std::fs::read(dir.path().join("normal.pdf")).unwrap());
    assert!(!pdfcraft_engine::print_safety::has_type3(safe.clone()).unwrap());
    assert!(pdfcraft_engine::print_safety::has_type3(normal.clone()).unwrap());
    let a = pdfcraft_cos::Document::open(safe).unwrap();
    let b = pdfcraft_cos::Document::open(normal).unwrap();
    let boxes = |d: &pdfcraft_cos::Document| {
        d.scan_objects()
            .filter_map(|(_, o)| o.as_dict().filter(|d| d.name(b"Type") == Some(b"Page")).and_then(|d| d.get(b"MediaBox")).cloned())
            .collect::<Vec<_>>()
    };
    assert_eq!(boxes(&a), boxes(&b));
    let script = json!([
        {"tool":"doc_open","args":{"path":"source.pdf"}},
        {"tool":"doc_print","args":{"doc":1,"path":"../escape-quality.pdf","print_compatibility":true}}
    ]);
    std::fs::write(dir.path().join("escape.json"), script.to_string()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_pdfcraft-cli"))
        .args(["run", "--script"])
        .arg(dir.path().join("escape.json"))
        .arg("--root")
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!dir.path().parent().unwrap().join("escape-quality.pdf").exists());
    assert_eq!(std::fs::read(dir.path().join("source.pdf")).unwrap(), original);
}

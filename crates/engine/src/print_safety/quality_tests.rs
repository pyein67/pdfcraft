//! Local-only paired print-quality oracle. No document or derived image is committed.
//! Exact pixel comparisons intentionally expose antialiasing differences; unlike the
//! general render oracle, these metrics do not forgive a one-pixel thickened stem.

use std::io::Cursor;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct Region {
    name: String,
    /// Original displayed-page coordinates, points from top left.
    rect: [f64; 4],
    #[serde(default)]
    exact: bool,
}

struct Gray {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl Gray {
    fn crop(&self, rect: [usize; 4]) -> Self {
        let [x0, y0, x1, y1] = rect;
        assert!(x0 < x1 && y0 < y1 && x1 <= self.width && y1 <= self.height, "ROI outside rendered page");
        let mut pixels = Vec::new();
        for y in y0..y1 {
            pixels.extend_from_slice(&self.pixels[y * self.width + x0..y * self.width + x1]);
        }
        Self { width: x1 - x0, height: y1 - y0, pixels }
    }

    fn stats(&self) -> serde_json::Value {
        assert_eq!(self.pixels.len(), self.width * self.height);
        let mut thresholds = [0usize; 3];
        let (mut horizontal, mut vertical, mut gray) = (0usize, 0usize, 0usize);
        let mut coverage = 0u64;
        for (i, &p) in self.pixels.iter().enumerate() {
            coverage += u64::from(255 - p);
            for (total, threshold) in thresholds.iter_mut().zip([64, 128, 192]) {
                *total += usize::from(p < threshold);
            }
            gray += usize::from(p > 0 && p < 255);
            if p < 128 {
                horizontal += usize::from(i % self.width == 0 || self.pixels[i - 1] >= 128);
                vertical += usize::from(i < self.width || self.pixels[i - self.width] >= 128);
            }
        }
        json!({
            "coverage_255":coverage, "threshold_ink":thresholds, "gray_pixels":gray,
            "horizontal_mean_run_px": if horizontal == 0 { 0.0 } else { thresholds[1] as f64 / horizontal as f64 },
            "vertical_mean_run_px": if vertical == 0 { 0.0 } else { thresholds[1] as f64 / vertical as f64 }
        })
    }

    fn profile(&self, x: usize) -> Vec<(usize, usize)> {
        assert!(x < self.width);
        let mut result = Vec::new();
        let mut start = None;
        for y in 0..=self.height {
            let black = y < self.height && self.pixels[y * self.width + x] < 128;
            match (start, black) {
                (None, true) => start = Some(y),
                (Some(s), false) => {
                    result.push((s, y));
                    start = None;
                }
                _ => {}
            }
        }
        result
    }
}

fn compare(a: &Gray, b: &Gray) -> serde_json::Value {
    assert_eq!((a.width, a.height), (b.width, b.height), "pair must have the same pixel grid");
    assert_eq!(a.pixels.len(), b.pixels.len());
    let mut absolute_error = 0u64;
    let mut different = 0usize;
    for (&a, &b) in a.pixels.iter().zip(&b.pixels) {
        let d = a.abs_diff(b);
        absolute_error += u64::from(d);
        different += usize::from(d > 20);
    }
    let profiles = [a.width / 4, a.width / 2, 3 * a.width / 4].map(|x| {
        let ar = a.profile(x);
        let br = b.profile(x);
        json!({"x":x,"identical_boundaries":ar==br,"reference_black_runs":ar.len(),"candidate_black_runs":br.len()})
    });
    json!({"pixels":a.pixels.len(),"different_gt20":different,"absolute_error":absolute_error,
        "reference":a.stats(),"candidate":b.stats(),"column_profiles":profiles})
}

fn native_content(doc: &pdfcraft_cos::Document) -> Vec<(String, Vec<u8>)> {
    let mut rows = Vec::new();
    for (_, object) in doc.scan_objects() {
        if let pdfcraft_cos::Object::Stream(stream) = object.as_ref() {
            let fields: &[&[u8]] = match stream.dict.name(b"Subtype") {
                Some(b"Image") => &[b"Width", b"Height", b"BitsPerComponent", b"ColorSpace", b"Decode", b"Interpolate", b"DecodeParms"],
                Some(b"Form") => &[b"BBox", b"Matrix"],
                _ => continue,
            };
            let metadata = fields.iter().map(|k| stream.dict.get(k).map(|o| doc.resolve(o))).collect::<Vec<_>>();
            rows.push((format!("{metadata:?}"), stream.raw.to_vec()));
        }
    }
    for page in pdfcraft_model::pages(doc) {
        let content = doc.resolve(page.dict.get(b"Contents").unwrap());
        let pdfcraft_cos::Object::Stream(stream) = content.as_ref() else { panic!("imposed sheet must have one content stream") };
        rows.push((format!("{:?}", page.dict.get(b"MediaBox")), stream.raw.to_vec()));
    }
    rows.sort();
    rows
}

fn render(path: &Path, out: &Path, dpi: u32) -> Gray {
    let result = Command::new("pdftoppm")
        .args(["-r", &dpi.to_string(), "-singlefile", "-gray", "-png"])
        .arg(path)
        .arg(out)
        .output()
        .expect("private quality oracle requires pdftoppm on PATH");
    assert!(result.status.success(), "pdftoppm failed: {}", String::from_utf8_lossy(&result.stderr));
    let bytes = std::fs::read(out.with_extension("png")).unwrap();
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    pixels.truncate(info.buffer_size());
    // Some Poppler versions encode -gray output as equal-channel RGB PNG.
    // Validate equality rather than introduce a second colour conversion.
    if info.color_type == png::ColorType::Rgb {
        let (rgb, tail) = pixels.as_chunks::<3>();
        assert!(tail.is_empty() && rgb.iter().all(|p| p[0] == p[1] && p[1] == p[2]));
        pixels = rgb.iter().map(|p| p[0]).collect();
    } else {
        assert_eq!(info.color_type, png::ColorType::Grayscale);
    }
    Gray { width: info.width as usize, height: info.height as usize, pixels }
}

#[test]
fn quality_metrics_expose_thickening_and_closed_barcode_gaps() {
    let a = Gray { width: 3, height: 5, pixels: vec![255, 255, 255, 0, 0, 0, 255, 255, 255, 0, 0, 0, 255, 255, 255] };
    let b = Gray { width: 3, height: 5, pixels: vec![255, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255] };
    let pair = compare(&a, &b);
    assert!(pair["candidate"]["coverage_255"].as_u64().unwrap() > pair["reference"]["coverage_255"].as_u64().unwrap());
    assert_eq!(a.profile(1), vec![(1, 2), (3, 4)]);
    assert_eq!(b.profile(1), vec![(1, 4)]);
    assert_eq!(pair["column_profiles"][1]["identical_boundaries"], false);
    assert_eq!(pair["different_gt20"], 3);
}

#[test]
fn quality_metrics_report_gray_edge_changes_without_claiming_a_size_change() {
    let a = Gray { width: 2, height: 2, pixels: vec![0, 255, 64, 255] };
    let b = Gray { width: 2, height: 2, pixels: vec![0, 255, 128, 255] };
    let pair = compare(&a, &b);
    assert_eq!(pair["absolute_error"], 64);
    assert_eq!(pair["different_gt20"], 1);
    assert_eq!(pair["reference"]["gray_pixels"], 1);
    assert_eq!(pair["candidate"]["gray_pixels"], 1);
    assert_eq!(compare(&a, &a)["absolute_error"], 0);
}

#[test]
#[ignore = "requires private PDF, ROI JSON and pdftoppm; never commit the inputs/outputs"]
fn private_print_quality_pairs_same_renderer_dpi_and_imposition() {
    let path = std::env::var_os("PDFCRAFT_PRIVATE_PRINT_PDF").expect("set PDFCRAFT_PRIVATE_PRINT_PDF");
    let region_path = std::env::var_os("PDFCRAFT_PRINT_QUALITY_REGIONS").expect("set PDFCRAFT_PRINT_QUALITY_REGIONS to a local JSON file");
    let regions: Vec<Region> = serde_json::from_slice(&std::fs::read(region_path).unwrap()).unwrap();
    assert!(!regions.is_empty() && regions.len() <= 32);
    let original = Arc::new(std::fs::read(path).unwrap());
    let doc = pdfcraft_cos::Document::open(original).unwrap();
    let pages = pdfcraft_model::pages(&doc);
    assert_eq!(pages.len(), 1, "this local oracle accepts one-page fixtures");
    let source_size = pages[0].display_size(&doc);
    let temp = tempfile::tempdir().unwrap();
    let mut rows = Vec::new();
    for (name, mode) in [("actual", crate::print::SizeMode::Actual), ("fit", crate::print::SizeMode::Fit)] {
        let settings =
            crate::print::Settings { pages: vec![0], paper: (595.28, 841.89), layout: crate::print::Layout::Size(mode), ..Default::default() };
        let sheets = crate::print::layout(&[source_size], &settings).unwrap();
        let m = sheets[0].placed[0].matrix.0;
        if name == "actual" {
            assert_eq!((m[0], m[3]), (1.0, 1.0));
        }
        assert_eq!(m[1], 0.0);
        assert_eq!(m[2], 0.0);
        let reference = Arc::new(crate::print::impose(&doc, &settings).unwrap());
        let (candidate, route) = super::compatibility_pdf(reference.clone(), false).unwrap();
        assert!(route.contains("Type 1C"), "fixture must use the outline route: {route}");
        let converted = pdfcraft_cos::Document::open(Arc::new(candidate.clone())).unwrap();
        let reference_doc = pdfcraft_cos::Document::open(reference.clone()).unwrap();
        assert!(native_content(&reference_doc) == native_content(&converted), "page/Form/image data or placement changed");
        let a_path = temp.path().join("reference.pdf");
        let b_path = temp.path().join("candidate.pdf");
        std::fs::write(&a_path, reference.as_slice()).unwrap();
        std::fs::write(&b_path, candidate).unwrap();
        for dpi in [300, 600] {
            let a = render(&a_path, &temp.path().join("reference"), dpi);
            let b = render(&b_path, &temp.path().join("candidate"), dpi);
            assert_eq!((a.width, a.height), (b.width, b.height), "page raster size changed");
            for region in &regions {
                let r = region.rect;
                assert!(r.iter().all(|v| v.is_finite()) && r[0] >= 0.0 && r[1] >= 0.0 && r[2] > r[0] && r[3] > r[1]);
                assert!(r[2] <= source_size.0 && r[3] <= source_size.1);
                // Display y is down; the imposition matrix uses PDF y-up coordinates.
                let dy = sheets[0].size.1 - source_size.1 * m[3] - m[5];
                let rect = [r[0] * m[0] + m[4], r[1] * m[3] + dy, r[2] * m[0] + m[4], r[3] * m[3] + dy];
                assert!(rect.iter().all(|v| *v >= 0.0));
                let rect = rect.map(|v| (v * f64::from(dpi) / 72.0).round() as usize);
                let metrics = compare(&a.crop(rect), &b.crop(rect));
                if region.exact {
                    assert_eq!(metrics["absolute_error"], 0, "region {} must retain exact pixels", region.name);
                }
                rows.push(json!({"layout":name,"dpi":dpi,"region":region.name,"matrix":m,"roi_pixels":rect,"metrics":metrics}));
            }
        }
    }
    let version = Command::new("pdftoppm").arg("-v").output().unwrap();
    let report = json!({"renderer":String::from_utf8_lossy(&version.stderr).lines().next(),"rows":rows,
        "limits":"PDF conversion comparison only; no Windows renderer, XPS or physical printer measured. Run widths are thresholded cross-section proxies, not physical stem measurements."});
    if let Some(path) = std::env::var_os("PDFCRAFT_PRINT_QUALITY_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
}

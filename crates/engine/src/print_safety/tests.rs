use super::*;

fn pdf(type3: bool, nested: bool) -> Vec<u8> {
    let mut doc = pdfcraft_create::blank(80.0, 60.0, 1).unwrap();
    let page = pdfcraft_model::pages(&doc)[0].obj;
    let mut font = Dict::new();
    font.set(b"Type".to_vec(), Object::name("Font"));
    font.set(b"Subtype".to_vec(), Object::name(if type3 { "Type3" } else { "Type1" }));
    if type3 {
        font.set(b"FontBBox".to_vec(), Object::Array(vec![0.into(), 0.into(), 1.into(), 1.into()]));
        font.set(b"FontMatrix".to_vec(), Object::Array(vec![1.into(), 0.into(), 0.into(), 1.into(), 0.into(), 0.into()]));
        font.set(b"FirstChar".to_vec(), 65i64);
        font.set(b"LastChar".to_vec(), 65i64);
        font.set(b"Widths".to_vec(), Object::Array(vec![1.into()]));
        let mut enc = Dict::new();
        enc.set(b"Differences".to_vec(), Object::Array(vec![65.into(), Object::name("a")]));
        font.set(b"Encoding".to_vec(), Object::Dict(enc));
        let glyph = doc.add(Object::Stream(Stream::flate(Dict::new(), b"1 0 d0 0 0 1 1 re f")));
        let mut procs = Dict::new();
        procs.set(b"a".to_vec(), Object::Ref(glyph));
        font.set(b"CharProcs".to_vec(), Object::Dict(procs));
    } else {
        font.set(b"BaseFont".to_vec(), Object::name("Helvetica"));
    }
    let font = doc.add(font);
    let mut fonts = Dict::new();
    fonts.set(b"F1".to_vec(), Object::Ref(font));
    let mut resources = Dict::new();
    resources.set(b"Font".to_vec(), Object::Dict(fonts));
    let content = b"1 0 0 rg 0 0 80 60 re f 0 0 0 rg BT /F1 20 Tf 10 10 Td (A) Tj ET";
    let content = if nested {
        let mut form = Dict::new();
        form.set(b"Type".to_vec(), Object::name("XObject"));
        form.set(b"Subtype".to_vec(), Object::name("Form"));
        form.set(b"BBox".to_vec(), Object::Array(vec![0.into(), 0.into(), 80.into(), 60.into()]));
        form.set(b"Resources".to_vec(), Object::Dict(resources));
        let form = doc.add(Object::Stream(Stream::flate(form, content)));
        resources = Dict::new();
        let mut xobjects = Dict::new();
        xobjects.set(b"Nested".to_vec(), Object::Ref(form));
        resources.set(b"XObject".to_vec(), Object::Dict(xobjects));
        b"/Nested Do".as_slice()
    } else {
        content.as_slice()
    };
    let content = doc.add(Object::Stream(Stream::flate(Dict::new(), content)));
    doc.update_dict(page, |d| {
        d.set(b"Resources".to_vec(), Object::Dict(resources));
        d.set(b"Contents".to_vec(), Object::Ref(content));
    })
    .unwrap();
    pdfcraft_cos::write_full(&doc, &SaveOptions::default()).unwrap()
}

#[test]
fn detects_compressed_nested_type3_but_not_ordinary_fonts_or_strings() {
    assert!(!has_type3(Arc::new(pdf(false, false))).unwrap());
    assert!(!has_type3(Arc::new(pdf(false, true))).unwrap());
    for nested in [false, true] {
        assert!(has_type3(Arc::new(pdf(true, nested))).unwrap());
    }
    let mut bytes = pdf(false, false);
    bytes.extend_from_slice(b"\n% /Subtype /Type3 is only a comment\n");
    assert!(!has_type3(Arc::new(bytes)).unwrap());
}

#[test]
fn image_print_removes_type3_preserves_dimensions_and_visible_ink() {
    let source = Arc::new(pdf(true, true));
    let saved = source.clone();
    let converted = Arc::new(image_pdf(source.clone()).unwrap());
    assert_eq!(*source, *saved, "the original document must not be edited");
    assert!(!has_type3(converted.clone()).unwrap());
    let doc = Document::open(converted.clone()).unwrap();
    let pages = pdfcraft_model::pages(&doc);
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].display_size(&doc), (80.0, 60.0));
    let mut renderer = PageRenderer::new(converted, Default::default());
    let rendered = renderer.render(RenderRequest { page: 0, scale: 1.0, ..Default::default() });
    assert!(rendered.error.is_none(), "{:?}", rendered.error);
    let pixels = rendered.rgba.as_chunks::<4>().0;
    assert!(pixels.iter().any(|p| p[0] > 240 && p[1] < 20 && p[2] < 20));
    assert!(pixels.iter().any(|p| p[0] < 20 && p[1] < 20 && p[2] < 20));
    for (_, object) in doc.scan_objects() {
        if let Some(d) = object.as_dict() {
            assert_ne!(d.name(b"Type"), Some(b"Font".as_slice()));
        }
    }
}

#[test]
fn image_print_rejects_oversized_or_invalid_jobs_without_downsampling() {
    assert!(image_pdf(Arc::new(b"not a PDF".to_vec())).is_err());
    for (w, h) in [(f64::NAN, 100.0), (f64::INFINITY, 1.0), (0.0, 100.0), (14400.0, 14400.0)] {
        assert!(raster_size(w, h).is_err());
    }
    assert_eq!(raster_size(612.0, 792.0).unwrap(), (5100, 6600));
}

#[test]
fn tiled_image_print_keeps_rotation_sheet_order_and_tile_edges() {
    let mut source = pdfcraft_create::blank(300.0, 270.0, 2).unwrap();
    for (i, page) in pdfcraft_model::pages(&source).iter().enumerate() {
        let color = if i == 0 { "1 0 0" } else { "0 0 1" };
        let content = format!("{color} rg 0 0 300 270 re f 0 0 0 rg 120 0 2 270 re f");
        let content = source.add(Object::Stream(Stream::flate(Dict::new(), content.as_bytes())));
        source
            .update_dict(page.obj, |d| {
                d.set(b"Contents".to_vec(), Object::Ref(content));
                d.set(b"Rotate".to_vec(), 90i64);
            })
            .unwrap();
    }
    let original = Arc::new(pdfcraft_cos::write_full(&source, &SaveOptions::default()).unwrap());
    let result = Arc::new(image_pdf(original.clone()).unwrap());
    let doc = Document::open(result.clone()).unwrap();
    assert_eq!(pdfcraft_model::pages(&doc).iter().map(|p| p.display_size(&doc)).collect::<Vec<_>>(), vec![(270.0, 300.0); 2]);
    let mut before = PageRenderer::new(original, Default::default());
    let mut after = PageRenderer::new(result, Default::default());
    for page in 0..2 {
        let request = RenderRequest { page, scale: 1.0, ..Default::default() };
        let a = before.render(request);
        let b = after.render(request);
        assert!(a.error.is_none() && b.error.is_none());
        assert_eq!((a.width, a.height), (b.width, b.height));
        // Include pixels around both 2048px tile boundaries (245.76pt) and the black stripe.
        let bad = a
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .zip(b.rgba.as_chunks::<4>().0)
            .filter(|(x, y)| x.iter().zip(y.iter()).any(|(a, b)| a.abs_diff(*b) > 20))
            .count();
        assert!(bad < 1000, "sheet {page}: {bad} visibly different pixels");
    }
}

#[test]
fn cyclic_resources_terminate_and_inherited_fonts_are_detected() {
    let mut doc = Document::open(Arc::new(pdf(true, false))).unwrap();
    let page = pdfcraft_model::pages(&doc)[0].obj;
    let resources = doc.get(page).as_dict().unwrap().get(b"Resources").unwrap().clone();
    let parent = doc.get(page).as_dict().unwrap().reference(b"Parent").unwrap();
    doc.update_dict(parent, |d| {
        d.set(b"Resources".to_vec(), resources);
    })
    .unwrap();
    doc.update_dict(page, |d| {
        d.remove(b"Resources");
    })
    .unwrap();
    assert!(type3_in(&doc).unwrap());
    let mut doc = pdfcraft_create::blank(80.0, 60.0, 1).unwrap();
    let page = pdfcraft_model::pages(&doc)[0].obj;
    let mut cycle = Dict::new();
    cycle.set(b"Self".to_vec(), Object::Ref(page));
    doc.update_dict(page, |d| {
        d.set(b"Resources".to_vec(), Object::Dict(cycle));
    })
    .unwrap();
    assert!(!type3_in(&doc).unwrap());
}

#[test]
#[ignore = "requires PDFCRAFT_PRIVATE_PRINT_PDF; never commit the private PDF"]
fn private_print_pdf_is_detected_and_flattens_without_fonts() {
    let path = std::env::var_os("PDFCRAFT_PRIVATE_PRINT_PDF").expect("set PDFCRAFT_PRIVATE_PRINT_PDF");
    let original = Arc::new(std::fs::read(&path).unwrap());
    let source = Document::open(original.clone()).unwrap();
    let count = pdfcraft_model::pages(&source).len();
    let settings = crate::print::Settings {
        pages: (0..count).collect(),
        paper: (595.28, 841.89),
        layout: crate::print::Layout::Size(crate::print::SizeMode::Actual),
        ..Default::default()
    };
    let imposed = Arc::new(crate::print::impose(&source, &settings).unwrap());
    assert!(has_type3(imposed.clone()).unwrap(), "compatibility routing must recognize this regression PDF after imposition");
    let converted = Arc::new(image_pdf(imposed).unwrap());
    assert!(!has_type3(converted.clone()).unwrap());
    let result = Document::open(converted).unwrap();
    assert_eq!(pdfcraft_model::pages(&result).len(), count);
    assert!(!result.scan_objects().any(|(_, o)| o.as_dict().is_some_and(|d| d.name(b"Type") == Some(b"Font"))));
    assert_eq!(*original, std::fs::read(path).unwrap());
}

fn outline_fixture(nested: bool) -> Arc<Vec<u8>> {
    let mut doc = Document::open(Arc::new(pdf(true, nested))).unwrap();
    let glyph = doc
        .scan_objects()
        .find_map(|(r, o)| match o.as_ref() {
            Object::Stream(s) if s.decoded().is_ok_and(|b| b.starts_with(b"1 0 d0")) => Some(r),
            _ => None,
        })
        .unwrap();
    // A hole and curved outline, at fractional coordinates; uncoloured single fill.
    doc.set(
        glyph,
        Object::Stream(Stream::flate(
            Dict::new(),
            b"1 0 0 0 1 1 d1 .01 0 0 .01 0 0 cm 0 0 m 83.125 0 l 100 0 100 100 83.125 100 c 0 100 l h 20 20 m 20 80 l 60 80 l 60 20 l h f",
        )),
    );
    Arc::new(pdfcraft_cos::write_full(&doc, &SaveOptions::default()).unwrap())
}

#[test]
fn outline_compatibility_preserves_geometry_curves_holes_and_other_content() {
    for nested in [false, true] {
        let source = outline_fixture(nested);
        let (bytes, route) = compatibility_pdf(source.clone(), false).unwrap();
        assert!(route.contains("Type 1C"), "{route}");
        let result = Arc::new(bytes);
        assert!(!has_type3(result.clone()).unwrap());
        let a = Document::open(source.clone()).unwrap();
        let b = Document::open(result.clone()).unwrap();
        assert_eq!(pdfcraft_model::pages(&a)[0].display_size(&a), pdfcraft_model::pages(&b)[0].display_size(&b));
        // All existing content/form streams must survive byte for byte; only font objects change.
        for (_, o) in a.scan_objects() {
            if let Object::Stream(s) = o.as_ref() {
                if matches!(s.dict.name(b"Type"), Some(b"ObjStm" | b"XRef")) {
                    continue;
                }
                let raw = s.decoded().unwrap();
                if !raw.starts_with(b"1 0 0 0 1 1 d1") {
                    assert!(b.scan_objects().any(|(_, o)| matches!(o.as_ref(), Object::Stream(t) if t.decoded().is_ok_and(|v| v == raw))));
                }
            }
        }
        assert!(b.scan_objects().any(|(_, o)| matches!(o.as_ref(), Object::Stream(s) if s.dict.name(b"Subtype") == Some(b"Type1C"))));
        let mut before = PageRenderer::new(source, Default::default());
        let mut after = PageRenderer::new(result, Default::default());
        let request = RenderRequest { scale: 4.0, ..Default::default() };
        let a = before.render(request);
        let b = after.render(request);
        assert!(a.error.is_none() && b.error.is_none());
        assert_eq!((a.width, a.height), (b.width, b.height));
        let bad = a.rgba.iter().zip(b.rgba.iter()).filter(|(a, b)| a.abs_diff(**b) > 20).count();
        assert!(bad < 300, "{bad} channels differ; missing fill, curve, hole or placement");
        let black = |r: &pdfcraft_render::RenderedPage| r.rgba.as_chunks::<4>().0.iter().filter(|p| p[0] < 20 && p[1] < 20 && p[2] < 20).count();
        assert!(black(&a) > 1000 && black(&b) > 1000);
    }
}

#[test]
fn ordinary_bytes_force_image_and_unsupported_type3_keep_their_contracts() {
    let source = Arc::new(pdf(false, false));
    let (bytes, route) = compatibility_pdf(source.clone(), false).unwrap();
    assert_eq!(*source, bytes);
    assert_eq!(route, "original vector");
    let (bytes, route) = compatibility_pdf(outline_fixture(false), true).unwrap();
    assert!(route.contains("explicit"));
    let doc = Document::open(Arc::new(bytes)).unwrap();
    assert!(!doc.scan_objects().any(|(_, o)| o.as_dict().is_some_and(|d| d.name(b"Type") == Some(b"Font"))));
    let (bytes, route) = compatibility_pdf(Arc::new(pdf(true, false)), false).unwrap();
    assert!(route.contains("fallback"));
    assert!(!has_type3(Arc::new(bytes)).unwrap());
    for body in [
        b"1 0 d0".as_slice(),
        b"1 0 0 0 1 1 d1 0 0 m 1 1 l S",
        b"1 0 0 0 1 1 d1 0 0 m 1 1 l f*",
        b"1 0 0 0 1 1 d1 /Im Do",
        b"1 0 0 0 1 1 d1 1 0 0 rg",
    ] {
        assert!(outlines::glyph(body, pdfcraft_content::Matrix::IDENTITY, 1000.0).is_err());
    }
}

#[test]
#[ignore = "requires PDFCRAFT_PRIVATE_PRINT_PDF; never commit the private PDF"]
fn private_print_outlines_preserve_page_and_native_elements() {
    let source = Arc::new(std::fs::read(std::env::var_os("PDFCRAFT_PRIVATE_PRINT_PDF").unwrap()).unwrap());
    let (bytes, route) = compatibility_pdf(source.clone(), false).unwrap();
    assert!(route.contains("Type 1C"), "{route}");
    let doc = Document::open(Arc::new(bytes.clone())).unwrap();
    let original = Document::open(source).unwrap();
    assert_eq!(pdfcraft_model::pages(&original)[0].display_size(&original), pdfcraft_model::pages(&doc)[0].display_size(&doc));
    if let Some(path) = std::env::var_os("PDFCRAFT_PRIVATE_PRINT_OUTPUT") {
        std::fs::write(path, bytes).unwrap();
    }
}

#[test]
fn outline_conversion_keeps_crop_rotation_text_spacing_and_normalized_advances() {
    let mut source = Document::open(outline_fixture(false)).unwrap();
    let page = pdfcraft_model::pages(&source)[0].obj;
    let font = source.scan_objects().find_map(|(r, o)| o.as_dict().filter(|d| d.name(b"Subtype") == Some(b"Type3")).map(|_| r)).unwrap();
    source
        .update_dict(font, |d| {
            d.set(b"FontMatrix".to_vec(), Object::Array([0.5, 0.0, 0.1, 0.5, 0.2, 0.1].map(Object::Real).to_vec()));
        })
        .unwrap();
    let content =
        source.add(Object::Stream(Stream::flate(Dict::new(), b"BT /F1 20 Tf 1 Tc 2 Tw 85 Tz 3 Ts 1 .1 -.1 1 10 10 Tm [(A) -50 (A)] TJ ET")));
    source
        .update_dict(page, |d| {
            d.set(b"Contents".to_vec(), Object::Ref(content));
            d.set(b"CropBox".to_vec(), Object::Array(vec![0.into(), (-5).into(), 70.into(), 55.into()]));
            d.set(b"Rotate".to_vec(), 90i64);
        })
        .unwrap();
    let original = Arc::new(pdfcraft_cos::write_full(&source, &SaveOptions::default()).unwrap());
    let (bytes, route) = compatibility_pdf(original.clone(), false).unwrap();
    assert!(route.contains("Type 1C"));
    let converted = Arc::new(bytes);
    let result = Document::open(converted.clone()).unwrap();
    let a = &pdfcraft_model::pages(&source)[0].dict;
    let b = pdfcraft_model::pages(&result).remove(0).dict;
    for key in [b"CropBox".as_slice(), b"MediaBox", b"Rotate"] {
        assert_eq!(a.get(key), b.get(key));
    }
    let converted_font = result.scan_objects().find_map(|(_, o)| o.as_dict().filter(|d| d.name(b"Subtype") == Some(b"Type1")).cloned()).unwrap();
    assert_eq!(converted_font.get(b"Widths").unwrap().as_array().unwrap()[0].as_f64(), Some(500.0));
    let a = source.resolve(a.get(b"Contents").unwrap());
    let b = result.resolve(b.get(b"Contents").unwrap());
    let decode = |o: &Object| match o {
        Object::Stream(s) => s.decoded().unwrap(),
        _ => panic!("missing content"),
    };
    assert_eq!(decode(&a), decode(&b), "TJ, spacing, rise and text transforms must survive unchanged");
    // Hayro's Type 3 renderer misplaces this transformed fixture; do not use it as
    // the oracle here. Independent MuPDF before/after agrees on the ink bounds.
}

#[test]
fn nonfill_text_mode_and_unbounded_outline_operands_use_fallback() {
    let mut doc = Document::open(outline_fixture(false)).unwrap();
    let page = pdfcraft_model::pages(&doc)[0].obj;
    let content = doc.add(Object::Stream(Stream::flate(Dict::new(), b"BT 1 Tr /F1 20 Tf 10 10 Td (A) Tj ET")));
    doc.update_dict(page, |d| d.set(b"Contents".to_vec(), Object::Ref(content))).unwrap();
    let (bytes, route) = compatibility_pdf(Arc::new(pdfcraft_cos::write_full(&doc, &SaveOptions::default()).unwrap()), false).unwrap();
    assert!(route.contains("fallback"));
    assert!(!has_type3(Arc::new(bytes)).unwrap());
    assert!(outlines::glyph(b"1 0 0 0 1 1 d1 1e30 1 m 2 2 l f", pdfcraft_content::Matrix::IDENTITY, 1000.0).is_err());
    assert!(outlines::glyph(b"1 0 0 0 1 1 d1 0 0 m 1 1 l f", pdfcraft_content::Matrix::IDENTITY, f64::NAN).is_err());
}

#[test]
fn unconverted_default_encoding_glyphs_never_disappear() {
    let mut doc = Document::open(outline_fixture(false)).unwrap();
    let font = doc.scan_objects().find_map(|(r, o)| o.as_dict().filter(|d| d.name(b"Subtype") == Some(b"Type3")).map(|_| r)).unwrap();
    let mut procs = doc.dict(doc.get(font).as_dict().unwrap().get(b"CharProcs").unwrap()).unwrap();
    // An extra procedure may be addressed by the base encoding outside FirstChar/LastChar.
    procs.set(b"b".to_vec(), procs.get(b"a").unwrap().clone());
    doc.update_dict(font, |d| d.set(b"CharProcs".to_vec(), Object::Dict(procs))).unwrap();
    let (bytes, route) = compatibility_pdf(Arc::new(pdfcraft_cos::write_full(&doc, &SaveOptions::default()).unwrap()), false).unwrap();
    assert!(route.contains("unmapped glyph procedures"), "{route}");
    assert!(!has_type3(Arc::new(bytes)).unwrap());
}

#[cfg(not(target_arch = "wasm32"))]
#[path = "quality_tests.rs"]
mod quality;

#[test]
fn compatibility_diagnostics_preserve_type1c_and_original_bytes() {
    let dir = tempfile::tempdir().unwrap();
    for input in [outline_fixture(false), Arc::new(pdf(false, false))] {
        let baseline = compatibility_pdf(input.clone(), false).unwrap();
        let path = dir.path().join(if baseline.1.contains("Type 1C") { "cff.txt" } else { "original.txt" });
        let observed = craft_print_diagnostics::with_context(
            craft_print_diagnostics::Context::from_setting(Some(path.clone().into_os_string()), &input),
            || compatibility_pdf(input.clone(), false).unwrap(),
        );
        assert_eq!(observed, baseline);
        let trace = std::fs::read_to_string(path).unwrap();
        assert!(trace.contains(if baseline.1.contains("Type 1C") { "type1c_conversion=true" } else { "type1c_conversion=false" }));
    }
}

#[test]
fn ab_keeps_type1c_bytes_and_refuses_image_fallback_without_changing_normal_mode() {
    let source = outline_fixture(false);
    let baseline = compatibility_pdf(source.clone(), false).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let context = craft_print_diagnostics::Context::from_setting(Some(dir.path().join("ab.txt").into_os_string()), &source)
        .unwrap()
        .with_ab(craft_print_diagnostics::ab::Mode::Off);
    craft_print_diagnostics::with_context(Some(context), || {
        assert_eq!(compatibility_pdf(source.clone(), false).unwrap(), baseline);
        assert!(compatibility_pdf(source, true).unwrap_err().contains("explicit image"));
        assert!(compatibility_pdf(Arc::new(pdf(true, false)), false).unwrap_err().contains("image fallback not used"));
        let ordinary = Arc::new(pdf(false, false));
        assert_eq!(compatibility_pdf(ordinary.clone(), false).unwrap().0, *ordinary);
    });
    // The existing crash-safe fallback remains available when the experiment is disabled.
    assert!(compatibility_pdf(Arc::new(pdf(true, false)), false).unwrap().1.contains("fallback"));
}

use super::*;

fn pdf(type3: bool, nested: bool) -> Vec<u8> {
    let mut doc = crate::tests::fixture(1);
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
fn normal_print_bytes_are_unchanged() {
    for nested in [false, true] {
        let bytes = Arc::new(pdf(false, nested));
        assert_eq!(*bytes, prepare(bytes.clone()).unwrap());
    }
}
#[test]
fn type3_is_removed_without_flattening_page_or_resampling_images() {
    for nested in [false, true] {
        let source = outline_fixture(nested);
        let mut doc = Document::open(source).unwrap();
        let page = pdfcraft_model::pages(&doc)[0].obj;
        let mut image = Dict::new();
        image.set(b"Subtype".to_vec(), Object::name("Image"));
        image.set(b"Width".to_vec(), 8i64);
        image.set(b"Height".to_vec(), 2i64);
        image.set(b"BitsPerComponent".to_vec(), 1i64);
        image.set(b"ColorSpace".to_vec(), Object::name("DeviceGray"));
        let image = doc.add(Object::Stream(Stream::flate(image, b"\xaa\x55")));
        doc.update_dict(page, |d| {
            let mut res = d.get(b"Resources").unwrap().as_dict().unwrap().clone();
            let mut xo = res.get(b"XObject").and_then(Object::as_dict).cloned().unwrap_or_default();
            xo.set(b"Barcode".to_vec(), Object::Ref(image));
            res.set(b"XObject".to_vec(), Object::Dict(xo));
            d.set(b"Resources".to_vec(), Object::Dict(res));
        })
        .unwrap();
        // Reference the synthetic barcode from page content, so garbage collection retains it.
        let content = doc.add(Object::Stream(Stream::flate(Dict::new(), b"q 8 0 0 2 50 40 cm /Barcode Do Q")));
        doc.update_dict(page, |d| {
            let original = d.get(b"Contents").unwrap().clone();
            d.set(b"Contents".to_vec(), Object::Array(vec![original, Object::Ref(content)]));
        })
        .unwrap();
        let bytes = Arc::new(pdfcraft_cos::write_full(&doc, &SaveOptions::default()).unwrap());
        let before = Document::open(bytes.clone()).unwrap();
        let converted = Arc::new(prepare(bytes).unwrap());
        assert!(!has_type3(converted.clone()).unwrap());
        let after = Document::open(converted).unwrap();
        assert_eq!(pdfcraft_model::pages(&before)[0].dict.get(b"MediaBox"), pdfcraft_model::pages(&after)[0].dict.get(b"MediaBox"));
        for (_, o) in before.scan_objects() {
            if let Object::Stream(stream) = o.as_ref()
                && (stream.dict.name(b"Subtype") == Some(b"Image")
                    || stream.decoded().is_ok_and(|b| b.starts_with(b"1 0 0 rg") || b.starts_with(b"q 8")))
            {
                assert!(after.scan_objects().any(|(_, v)| matches!(v.as_ref(),Object::Stream(s) if s==stream)));
            }
        }
        assert!(after.scan_objects().any(|(_, o)| o.as_dict().is_some_and(|d| d.name(b"Subtype") == Some(b"Type1C"))));
    }
}
#[test]
fn unsupported_type3_is_refused_instead_of_silently_rasterizing() {
    let source = outline_fixture(false);
    let mut doc = Document::open(source).unwrap();
    let glyph = doc
        .scan_objects()
        .find_map(|(r, o)| matches!(o.as_ref(),Object::Stream(s) if s.decoded().is_ok_and(|b|b.starts_with(b"1 0 0 0"))).then_some(r))
        .unwrap();
    doc.set(glyph, Object::Stream(Stream::flate(Dict::new(), b"1 0 d0 1 0 0 rg 0 0 1 1 re f")));
    let bytes = Arc::new(pdfcraft_cos::write_full(&doc, &SaveOptions::default()).unwrap());
    assert!(prepare(bytes).unwrap_err().contains("Nothing sent"));
}
/// Optional private local oracle. No document, text, path or derived page is committed.
#[test]
#[ignore = "requires explicit private PDF and output paths; no printing"]
fn local_private_pdf_vector_preparation() {
    let input = std::env::var_os("PDFCRAFT_PRIVATE_PRINT_PDF").unwrap();
    let output = std::env::var_os("PDFCRAFT_PRIVATE_PREPARED_PDF").unwrap();
    let bytes = std::fs::read(input).unwrap();
    let doc = Document::open(Arc::new(bytes.clone())).unwrap();
    let sizes = pdfcraft_model::pages(&doc).iter().map(|p| p.dict.get(b"MediaBox").cloned()).collect::<Vec<_>>();
    assert!(has_type3(Arc::new(bytes.clone())).unwrap());
    let prepared = prepare(Arc::new(bytes.clone())).unwrap();
    let after = Document::open(Arc::new(prepared.clone())).unwrap();
    assert_eq!(sizes, pdfcraft_model::pages(&after).iter().map(|p| p.dict.get(b"MediaBox").cloned()).collect::<Vec<_>>());
    for (_, o) in doc.scan_objects() {
        if let Object::Stream(image) = o.as_ref()
            && image.dict.name(b"Subtype") == Some(b"Image")
        {
            assert!(after.scan_objects().any(|(_, o)| matches!(o.as_ref(),Object::Stream(s) if s==image)));
        }
    }
    for mode in [crate::SizeMode::Fit, crate::SizeMode::Actual] {
        let paper = crate::PAPERS.iter().find(|(n, _)| *n == "A4").unwrap().1;
        let settings = crate::Settings { pages: vec![0], paper, layout: crate::Layout::Size(mode), ..Default::default() };
        let imposed = Arc::new(crate::impose(&doc, &settings).unwrap());
        let before = Document::open(imposed.clone()).unwrap();
        let safe = Arc::new(prepare(imposed).unwrap());
        assert!(!has_type3(safe.clone()).unwrap());
        let after = Document::open(safe).unwrap();
        assert_eq!(pdfcraft_model::pages(&before)[0].display_size(&before), pdfcraft_model::pages(&after)[0].display_size(&after));
        for (_, o) in before.scan_objects() {
            if let Object::Stream(image) = o.as_ref()
                && image.dict.name(b"Subtype") == Some(b"Image")
            {
                assert!(after.scan_objects().any(|(_, o)| matches!(o.as_ref(),Object::Stream(s) if s==image)));
            }
        }
    }
    std::fs::OpenOptions::new().write(true).create_new(true).open(output).and_then(|mut f| std::io::Write::write_all(&mut f, &prepared)).unwrap();
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
    let bytes = prepare(original.clone()).unwrap();
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
fn curves_holes_and_placement_keep_visible_ink() {
    use pdfcraft_render::{PageRenderer, RenderRequest};
    let bytes = outline_fixture(true);
    let converted = Arc::new(prepare(bytes.clone()).unwrap());
    let mut original = PageRenderer::new(bytes, Default::default());
    let mut after = PageRenderer::new(converted, Default::default());
    let request = RenderRequest { scale: 4.0, ..Default::default() };
    let a = original.render(request);
    let b = after.render(request);
    assert!(a.error.is_none() && b.error.is_none());
    assert_eq!((a.width, a.height), (b.width, b.height));
    let different = a.rgba.iter().zip(b.rgba.iter()).filter(|(a, b)| a.abs_diff(**b) > 20).count();
    assert!(different < 300, "{different} channels changed: outline/hole/placement regression");
}

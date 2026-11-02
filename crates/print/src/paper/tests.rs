use super::*;
fn paper(name: &str, size: (f64, f64)) -> Paper {
    Paper { name: name.into(), size }
}
#[test]
fn korean_windows_factory_only_and_no_scale_changes() {
    let a4 = super::super::PAPERS.iter().position(|(n, _)| *n == "A4").unwrap();
    for tag in ["ko-KR", "ko_KR.UTF-8", "KO", "ko"] {
        assert_eq!(initial(true, Some(tag)), a4);
    }
    for tag in [None, Some("en-US"), Some("kok-IN")] {
        assert_eq!(initial(true, tag), 0);
    }
    assert_eq!(initial(false, Some("ko-KR")), 0);
    assert_eq!(super::super::Settings::default().layout, super::super::Layout::Size(super::super::SizeMode::Fit));
}
#[test]
fn saved_stock_precedes_locale_and_unsupported_a4_uses_valid_driver_media() {
    let a4 = paper("A4", (595.28, 841.89));
    let letter = paper("Letter", (612., 792.));
    let label = paper("Label", (288., 432.));
    let caps = Capabilities { papers: vec![a4.clone(), label.clone()], default: Some(0) };
    assert_eq!(resolve(&label, &caps).unwrap(), label);
    assert_eq!(resolve(&a4, &caps).unwrap(), a4);
    let caps = Capabilities { papers: vec![letter.clone(), label.clone()], default: Some(1) };
    assert_eq!(resolve(&a4, &caps).unwrap(), label);
    assert_eq!(resolve(&a4, &Capabilities { papers: vec![letter.clone()], default: None }).unwrap(), letter);
    assert!(resolve(&a4, &Capabilities { papers: vec![], default: None }).is_err());
    assert!(resolve(&a4, &Capabilities { papers: vec![paper("invalid", (f64::NAN, 0.))], default: Some(0) }).is_err());
}
#[test]
fn default_ticket_dimensions_are_namespace_aware() {
    let xml=br#"<f:PrintTicket xmlns:f="http://schemas.microsoft.com/windows/2003/08/printing/printschemaframework" xmlns:k="http://schemas.microsoft.com/windows/2003/08/printing/printschemakeywords"><f:Feature name="k:PageMediaSize"><f:Option><f:ScoredProperty name="k:MediaSizeWidth"><f:Value>210000</f:Value></f:ScoredProperty><f:ScoredProperty name="k:MediaSizeHeight"><f:Value>297000</f:Value></f:ScoredProperty></f:Option></f:Feature></f:PrintTicket>"#;
    let size = default_size(xml).unwrap();
    assert!((size.0 - 595.27559).abs() < 0.0001);
    assert!((size.1 - 841.88976).abs() < 0.0001);
    assert!(default_size(b"not xml").is_none());
}

#[test]
fn failed_default_probe_does_not_discard_a_valid_media_catalog() {
    let a4 = paper("A4", (595.28, 841.89));
    let caps =
        read_catalog(|| Ok(vec![a4.clone()]), || Err(ReadFailure { phase: "defaults.merge", kind: "win32", hresult: Some(0x80070057u32 as i32) }))
            .unwrap();
    assert_eq!(caps.default, None);
    assert_eq!(resolve(&a4, &caps).unwrap(), a4);
    assert_eq!(resolve(&paper("Label", (200., 300.)), &caps).unwrap(), a4);
}
#[test]
fn media_failure_preserves_the_stage_and_hresult_and_skips_default_probe() {
    let error = ReadFailure { phase: "capabilities.get", kind: "win32", hresult: Some(0x800401f0u32 as i32) };
    let result = read_catalog(|| Err(error.clone()), || panic!("default probe must not run"));
    assert_eq!(result.unwrap_err(), error);
    assert_eq!(error.to_string(), "phase=capabilities.get kind=win32 hresult=0x800401F0");
    // Diagnostic codes carry no arbitrary driver error message or XML.
    assert!(!error.to_string().contains("<"));
}
#[test]
fn empty_or_invalid_media_is_unknown_instead_of_a_claimed_supported_a4() {
    let result = read_catalog(|| Ok(vec![paper("invalid", (0., f64::NAN))]), || panic!("no valid media"));
    assert_eq!(result.unwrap_err().phase, "media.list");
}

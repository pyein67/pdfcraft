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

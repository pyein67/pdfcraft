use super::ab::*;
use super::*;
fn ticket() -> String {
    format!(
        r#"<f:PrintTicket version="1" xmlns:f="{PSF}" xmlns:k="{PSK}" xmlns:s="{SAMSUNG}" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
<f:Feature name="k:PageMediaSize"><f:Option name="k:ISOA4"><f:ScoredProperty name="k:MediaSizeWidth"><f:Value>210000</f:Value></f:ScoredProperty><f:ScoredProperty name="k:MediaSizeHeight"><f:Value>297000</f:Value></f:ScoredProperty></f:Option></f:Feature>
<f:Feature name="k:PageResolution"><f:Option name="s:1_600x600_dpi"><f:ScoredProperty name="k:ResolutionX"><f:Value>600</f:Value></f:ScoredProperty><f:ScoredProperty name="k:ResolutionY"><f:Value>600</f:Value></f:ScoredProperty></f:Option></f:Feature>
<f:Feature name="k:PageScaling"><f:Option name="k:None"/></f:Feature>
<f:Feature name="k:PageOutputColor"><f:Option name="k:Monochrome"/></f:Feature>
<f:Feature name="s:DocumentFineEdge"><f:Option name="s:Normal"/></f:Feature>
<f:Feature name="s:DocumentDarkenText"><f:Option name="s:Minimum"><f:Property name="s:Value"><f:Value>3</f:Value></f:Property></f:Option></f:Feature>
<f:Feature name="s:DoNotLog"><f:Option name="s:Private"><f:Property name="s:Label"><f:Value>PRIVATE_DOCUMENT_TITLE_12345</f:Value></f:Property></f:Option></f:Feature>
</f:PrintTicket>"#
    )
}
fn caps() -> String {
    format!(
        r#"<f:PrintCapabilities xmlns:f="{PSF}" xmlns:s="{SAMSUNG}"><f:Feature name="s:DocumentDarkenText"><f:Option name="s:Minimum"><f:Property name="s:Value"><f:Value>3</f:Value></f:Property></f:Option><f:Option name="s:Off"><f:Property name="s:Value"><f:Value>0</f:Value></f:Property></f:Option></f:Feature></f:PrintCapabilities>"#
    )
}
fn dm() -> Vec<u8> {
    let mut b = vec![0; 220];
    b[68..70].copy_from_slice(&220u16.to_le_bytes());
    b[72..76].copy_from_slice(&0x400u32.to_le_bytes());
    b[90..92].copy_from_slice(&600i16.to_le_bytes());
    b
}
fn context(path: &std::path::Path, mode: Mode) -> Context {
    let mut c = Context::from_setting(Some(path.into()), b"opaque test bytes").unwrap();
    c.ab_mode = Some(mode);
    c
}
#[test]
fn selector_is_default_off_and_requires_audit_sink() {
    assert_eq!(parse(None, None).unwrap(), None);
    assert_eq!(parse(Some(""), None).unwrap(), None);
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("log.txt");
    assert_eq!(parse(Some("Minimum"), Some(&p)).unwrap(), Some(Mode::Minimum));
    assert_eq!(parse(Some("off"), Some(&p)).unwrap(), Some(Mode::Off));
    for m in ["disable", "1200", "Off\n"] {
        assert!(parse(Some(m), Some(&p)).is_err());
    }
    assert!(parse(Some("Off"), None).is_err());
    assert!(parse(Some("Off"), Some(std::path::Path::new("relative.txt"))).is_err());
}
#[test]
fn minimum_preserves_exact_ticket_and_off_changes_only_advertised_feature() {
    let base = ticket();
    assert_eq!(request(&base, &caps(), Mode::Minimum).unwrap(), base);
    let scored = request(&base, &caps().replace("f:Property", "f:ScoredProperty"), Mode::Off).unwrap();
    assert!(scored.contains("<psf:ScoredProperty name=\"s:Value\">"));
    let off = request(&base, &caps(), Mode::Off).unwrap();
    assert!(off.contains("name=\"s:Off\""));
    assert!(base.contains("s:Minimum"));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log.txt");
    with_context(Some(context(&path, Mode::Off)), || {
        verify_candidate(&base, &off, &off, Mode::Off).unwrap();
        observe_validated_ticket(off.replace("k:ISOA4", "s:PRIVATE_PERSON_NAME").as_bytes());
    });
    let log = std::fs::read_to_string(path).unwrap();
    assert!(!log.contains("PRIVATE_DOCUMENT_TITLE_12345"));
    assert!(!log.contains("PRIVATE_PERSON_NAME"));
    // Numeric option values are copied from capabilities, not assumed from enum ordinal.
    assert!(request(&base, &caps().replace(">0<", ">17<"), Mode::Off).unwrap().contains(">17<"));
}
#[test]
fn rejects_reverted_changed_absent_and_unsupported_requests_without_success_claim() {
    let base = ticket();
    let off = request(&base, &caps(), Mode::Off).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log.txt");
    with_context(Some(context(&path, Mode::Off)), || {
        assert!(verify_candidate(&base, &off, &base, Mode::Off).unwrap_err().contains("reverted"));
        for returned in [
            off.replace(">600<", ">1200<"),
            off.replace("k:Monochrome", "k:Color"),
            off.replace("s:Normal", "s:Off"),
            off.replace("PRIVATE_DOCUMENT_TITLE_12345", "changed"),
            off.replace("k:None", "k:CustomSquare"),
        ] {
            assert!(verify_candidate(&base, &off, &returned, Mode::Off).is_err());
        }
        let missing = off.replace("s:DocumentDarkenText", "s:Unknown");
        assert!(verify_candidate(&base, &off, &missing, Mode::Off).is_err());
    });
    let log = std::fs::read_to_string(path).unwrap();
    assert!(log.contains("observed=Some((Minimum, 3))"));
    assert!(!log.contains("confirmed_ticket=true"));
    assert!(log.contains("NOT_PRINTED"));
    for cap in [
        caps().replace("s:Off", "s:Unknown"),
        caps().replace(SAMSUNG, "urn:foreign"),
        caps().replace("<f:Value>0</f:Value>", "<f:ParameterRef name=\"s:secret\"/>"),
    ] {
        assert!(request(&base, &cap, Mode::Off).is_err());
    }
}
#[test]
fn normalization_confirms_only_ticket_and_freezes_other_settings_and_public_devmode() {
    let base = ticket();
    let off = request(&base, &caps(), Mode::Off).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log.txt");
    with_context(Some(context(&path, Mode::Off)), || {
        remember_baseline(&base, &base, &dm()).unwrap();
        verify_normalized(&off, &off, &dm(), Mode::Off).unwrap();
        verify_raster_dpi(600.0).unwrap();
        assert!(verify_normalized(&off, &base, &dm(), Mode::Off).is_err());
        assert!(verify_normalized(&off, &off.replace("s:Normal", "s:Off"), &dm(), Mode::Off).is_err());
        let mut changed = dm();
        changed[86..88].copy_from_slice(&2u16.to_le_bytes());
        assert!(verify_normalized(&off, &off, &changed, Mode::Off).is_err());
        changed[90..92].copy_from_slice(&1200i16.to_le_bytes());
        assert!(verify_normalized(&off, &off, &changed, Mode::Off).is_err());
        let mut scaled = dm();
        scaled[72..76].copy_from_slice(&0x410u32.to_le_bytes());
        scaled[84..86].copy_from_slice(&94i16.to_le_bytes());
        assert!(verify_normalized(&off, &off, &scaled, Mode::Off).is_err());
        assert!(verify_raster_dpi(1200.0).is_err());
    });
    let log = std::fs::read_to_string(path).unwrap();
    assert!(log.contains("physical_effect=UNVERIFIED"));
    assert!(log.contains("observed_dpi_x=Some(600) observed_dpi_y=Some(600)"));
    assert!(!log.contains("PRIVATE_DOCUMENT_TITLE_12345"));
}
#[test]
fn missing_baseline_or_audit_failure_stops_experiment() {
    let base = ticket();
    let off = request(&base, &caps(), Mode::Off).unwrap();
    let dir = tempfile::tempdir().unwrap();
    with_context(Some(context(&dir.path().join("missing.txt"), Mode::Off)), || {
        assert!(verify_normalized(&off, &off, &dm(), Mode::Off).is_err());
    });
    with_context(Some(context(dir.path(), Mode::Off)), || {
        assert!(verify_candidate(&base, &off, &off, Mode::Off).is_err());
    });
}
#[test]
fn prefix_rewrites_do_not_fake_changes_and_malformed_inputs_are_bounded() {
    let base = ticket();
    let off = request(&base, &caps(), Mode::Off).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log.txt");
    let rewritten = off.replace("xmlns:s=", "xmlns:vendor=").replace("\"s:", "\"vendor:");
    with_context(Some(context(&path, Mode::Off)), || {
        verify_candidate(&base, &off, &rewritten, Mode::Off).unwrap();
    });
    let nested = format!("{}0{}", "<f:Property name=\"s:deep\">".repeat(70), "</f:Property>".repeat(70));
    let deep = off.replace("</f:PrintTicket>", &format!("{nested}</f:PrintTicket>"));
    with_context(Some(context(&dir.path().join("deep.txt"), Mode::Off)), || {
        assert!(verify_candidate(&base, &off, &deep, Mode::Off).is_err());
    });
    assert!(request(&"x".repeat(4 * 1024 * 1024 + 1), &caps(), Mode::Off).is_err());
    assert!(request("<!DOCTYPE f [<!ENTITY x SYSTEM 'file:///private'>]><f>&x;</f>", &caps(), Mode::Off).is_err());
}

#[test]
fn parameter_references_require_unique_explicit_values() {
    let base = ticket();
    let parameterized = base
        .replace("<f:Value>600</f:Value>", "<f:ParameterRef name=\"s:Dpi\"/>")
        .replace("</f:PrintTicket>", "<f:ParameterInit name=\"s:Dpi\"><f:Value>600</f:Value></f:ParameterInit></f:PrintTicket>");
    assert!(request(&parameterized, &caps(), Mode::Off).is_ok());
    assert!(request(&parameterized.replace("<f:Value>600</f:Value>", "<f:Value>1200</f:Value>"), &caps(), Mode::Off).is_err());
    assert!(
        request(&parameterized.replace("<f:ParameterInit name=\"s:Dpi\"><f:Value>600</f:Value></f:ParameterInit>", ""), &caps(), Mode::Off).is_err()
    );
}

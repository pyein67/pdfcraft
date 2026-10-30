//! Samsung DarkenText experiment: pure validation and job-local state, never queue APIs.
use crate::{capture, devmode, record_required};
use roxmltree::{Document, Node};
use std::{collections::BTreeMap, path::Path, sync::Mutex};

pub const PSF: &str = "http://schemas.microsoft.com/windows/2003/08/printing/printschemaframework";
pub const PSK: &str = "http://schemas.microsoft.com/windows/2003/08/printing/printschemakeywords";
pub const SAMSUNG: &str = "http://schemas.microsoft.com/windows/2003/08/printing/SAMSUNGPRINTER";
const LIMIT: usize = 4 * 1024 * 1024;
/// Only the observed Samsung options, not a general printer configuration API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Minimum,
    Off,
}
impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Minimum => "Minimum",
            Self::Off => "Off",
        }
    }
}
/// Strict opt-in. Invalid selectors or a missing diagnostic sink refuse the experiment.
pub fn parse(mode: Option<&str>, log: Option<&Path>) -> Result<Option<Mode>, String> {
    let Some(mode) = mode.filter(|v| !v.is_empty()) else { return Ok(None) };
    let mode = if mode.eq_ignore_ascii_case("minimum") {
        Mode::Minimum
    } else if mode.eq_ignore_ascii_case("off") {
        Mode::Off
    } else {
        return Err("A/B selector must be Minimum or Off; nothing sent".into());
    };
    if !log.is_some_and(Path::is_absolute) {
        return Err("A/B requires an absolute PDFCRAFT_PRINT_DIAGNOSTICS file path; nothing sent".into());
    }
    Ok(Some(mode))
}
/// Read environment without mutating it. The existing isolated worker inherits these settings.
pub fn configured() -> Result<Option<Mode>, String> {
    let mode = std::env::var("PDFCRAFT_PRINT_AB_DARKEN_TEXT").map(Some).or_else(|e| match e {
        std::env::VarError::NotPresent => Ok(None),
        _ => Err("Invalid A/B selector encoding; nothing sent".to_owned()),
    })?;
    let log = std::env::var_os("PDFCRAFT_PRINT_DIAGNOSTICS").map(std::path::PathBuf::from);
    parse(mode.as_deref(), log.as_deref())
}
/// Captured setting for this actual submission, including its spool thread.
pub fn mode() -> Option<Mode> {
    capture().and_then(|c| c.ab_mode)
}
#[derive(Default)]
pub(crate) struct State {
    baseline: Mutex<Option<Baseline>>,
}
struct Baseline {
    protected: String,
    public_devmode: Vec<u8>,
}

fn xml(text: &str) -> Result<Document<'_>, String> {
    if text.len() > LIMIT {
        return Err("A/B ticket exceeds size limit; nothing sent".into());
    }
    Document::parse_with_options(text, roxmltree::ParsingOptions { nodes_limit: 100_000, ..Default::default() })
        .map_err(|_| "A/B ticket XML is invalid; nothing sent".into())
}
fn qname(node: Node<'_, '_>, text: &str) -> String {
    let (prefix, local) = text.split_once(':').unwrap_or(("", text));
    format!("{{{}}}{local}", node.lookup_namespace_uri(if prefix.is_empty() { None } else { Some(prefix) }).unwrap_or(""))
}
fn named(node: Node<'_, '_>, ns: &str, name: &str) -> bool {
    node.attribute("name").is_some_and(|s| qname(node, s) == format!("{{{ns}}}{name}"))
}
fn darken(node: Node<'_, '_>) -> bool {
    node.has_tag_name((PSF, "Feature")) && named(node, SAMSUNG, "DocumentDarkenText")
}
fn feature<'a, 'input>(doc: &'a Document<'input>, ns: &str, name: &str) -> Result<Node<'a, 'input>, String> {
    let nodes: Vec<_> = doc.root_element().children().filter(|n| n.has_tag_name((PSF, "Feature")) && named(*n, ns, name)).collect();
    if nodes.len() != 1 {
        return Err(format!("A/B feature {name} missing or ambiguous; nothing sent"));
    }
    nodes.first().copied().ok_or_else(|| "A/B feature unavailable; nothing sent".into())
}
fn choice(node: Node<'_, '_>) -> Result<String, String> {
    let mut options = node.children().filter(|n| n.has_tag_name((PSF, "Option")));
    let one = options.next().ok_or("A/B selected option missing; nothing sent")?;
    if options.next().is_some() {
        return Err("A/B selected option ambiguous; nothing sent".into());
    }
    Ok(qname(one, one.attribute("name").unwrap_or("")))
}
fn number(node: Node<'_, '_>, ns: &str, name: &str) -> Option<i64> {
    let mut props =
        node.descendants().filter(|n| (n.has_tag_name((PSF, "ScoredProperty")) || n.has_tag_name((PSF, "Property"))) && named(*n, ns, name));
    let prop = props.next()?;
    if props.next().is_some() {
        return None;
    }
    let value = if let Some(value) = prop.children().find(|n| n.has_tag_name((PSF, "Value"))) {
        value
    } else {
        let reference = prop.children().find(|n| n.has_tag_name((PSF, "ParameterRef")))?;
        let key = qname(reference, reference.attribute("name")?);
        let mut inits = prop
            .document()
            .root_element()
            .children()
            .filter(|n| n.has_tag_name((PSF, "ParameterInit")) && n.attribute("name").is_some_and(|v| qname(*n, v) == key));
        let init = inits.next()?;
        if inits.next().is_some() {
            return None;
        }
        init.children().find(|n| n.has_tag_name((PSF, "Value")))?
    };
    value.text()?.trim().parse().ok()
}
/// Canonical comparison stays in memory; no raw XML, labels or arbitrary values reach logs.
fn canonical(node: Node<'_, '_>, depth: usize) -> Result<String, String> {
    if depth > 64 {
        return Err("A/B XML nesting limit; nothing sent".into());
    }
    let mut attrs = BTreeMap::new();
    for a in node.attributes() {
        let value = if a.name() == "name" || a.name() == "type" { qname(node, a.value()) } else { a.value().to_owned() };
        attrs.insert(format!("{{{}}}{}", a.namespace().unwrap_or(""), a.name()), value);
    }
    let mut children = Vec::new();
    let mut child_bytes = 0usize;
    for child in node.children().filter(Node::is_element) {
        // DisplayName is capability/UI metadata, not an output setting.
        if (child.has_tag_name((PSF, "Property")) || child.has_tag_name((PSF, "ScoredProperty"))) && named(child, PSK, "DisplayName") {
            continue;
        }
        let child = canonical(child, depth + 1)?;
        child_bytes = child_bytes.checked_add(child.len()).filter(|v| *v <= LIMIT).ok_or("A/B comparison limit; nothing sent")?;
        children.push(child);
    }
    children.sort();
    let text = node.children().filter(Node::is_text).filter_map(|n| n.text()).collect::<String>();
    let text = text.trim();
    let text = if text.contains(':') && node.lookup_namespace_uri(text.split(':').next()).is_some() { qname(node, text) } else { text.to_owned() };
    let result = format!("{:?}{attrs:?}{text:?}{children:?}", (node.tag_name().namespace(), node.tag_name().name()));
    if result.len() > LIMIT {
        return Err("A/B comparison limit; nothing sent".into());
    }
    Ok(result)
}
fn protected(doc: &Document<'_>) -> Result<String, String> {
    if !doc.root_element().has_tag_name((PSF, "PrintTicket")) {
        return Err("A/B requires a PrintTicket; nothing sent".into());
    }
    let mut parts = Vec::new();
    let mut part_bytes = 0usize;
    for n in doc.root_element().children().filter(Node::is_element).filter(|n| !darken(*n)) {
        let part = canonical(n, 0)?;
        part_bytes = part_bytes.checked_add(part.len()).filter(|v| *v <= LIMIT).ok_or("A/B comparison limit; nothing sent")?;
        parts.push(part);
    }
    parts.sort();
    let result = format!("{parts:?}");
    if result.len() > LIMIT {
        return Err("A/B comparison limit; nothing sent".into());
    }
    Ok(result)
}
fn selection(doc: &Document<'_>) -> Result<(Mode, i64), String> {
    let f = feature(doc, SAMSUNG, "DocumentDarkenText")?;
    let option = choice(f)?;
    let mode = if option == format!("{{{SAMSUNG}}}Minimum") {
        Mode::Minimum
    } else if option == format!("{{{SAMSUNG}}}Off") {
        Mode::Off
    } else {
        return Err("A/B DarkenText is neither Minimum nor Off; nothing sent".into());
    };
    Ok((mode, number(f, SAMSUNG, "Value").ok_or("A/B DarkenText value unavailable; nothing sent")?))
}
fn core(doc: &Document<'_>) -> Result<(), String> {
    if !doc.root_element().has_tag_name((PSF, "PrintTicket")) {
        return Err("A/B requires a PrintTicket; nothing sent".into());
    }
    let media = feature(doc, PSK, "PageMediaSize")?;
    if number(media, PSK, "MediaSizeWidth") != Some(210000) || number(media, PSK, "MediaSizeHeight") != Some(297000) {
        return Err("A/B requires A4; nothing sent".into());
    }
    let dpi = feature(doc, PSK, "PageResolution")?;
    if number(dpi, PSK, "ResolutionX") != Some(600) || number(dpi, PSK, "ResolutionY") != Some(600) {
        return Err("A/B requires existing 600x600 DPI; DPI is not changed; nothing sent".into());
    }
    if choice(feature(doc, PSK, "PageScaling")?)? != format!("{{{PSK}}}None") {
        return Err("A/B requires PageScaling None; nothing sent".into());
    }
    if ![format!("{{{PSK}}}Monochrome"), format!("{{{PSK}}}Grayscale")].contains(&choice(feature(doc, PSK, "PageOutputColor")?)?) {
        return Err("A/B requires black/white or grayscale; nothing sent".into());
    }
    if choice(feature(doc, SAMSUNG, "DocumentFineEdge")?)? != format!("{{{SAMSUNG}}}Normal") {
        return Err("A/B requires existing FineEdge Normal; nothing sent".into());
    }
    Ok(())
}
fn audit(stage: &'static str, args: std::fmt::Arguments<'_>) -> Result<(), String> {
    record_required(stage, args).map_err(|_| "A/B diagnostic log unavailable; nothing sent".into())
}
/// Construct only the DarkenText delta from advertised Samsung values; preserve all other XML.
pub fn request(base: &str, caps: &str, mode: Mode) -> Result<String, String> {
    let base_doc = xml(base)?;
    core(&base_doc)?;
    if selection(&base_doc)?.0 != Mode::Minimum {
        return Err("A/B baseline is not Minimum; nothing sent".into());
    }
    let caps_doc = xml(caps)?;
    if !caps_doc.root_element().has_tag_name((PSF, "PrintCapabilities")) {
        return Err("A/B capabilities unavailable; nothing sent".into());
    }
    let f = feature(&caps_doc, SAMSUNG, "DocumentDarkenText")?;
    let choices: Vec<_> = f.children().filter(|n| n.has_tag_name((PSF, "Option")) && named(*n, SAMSUNG, mode.name())).collect();
    if choices.len() != 1 {
        return Err("A/B requested Samsung option unsupported/ambiguous; nothing sent".into());
    }
    let option = choices.first().copied().ok_or("A/B option unavailable; nothing sent")?;
    if option.attributes().any(|a| a.name() == "constrained" && qname(option, a.value()) != format!("{{{PSK}}}None")) {
        return Err("A/B requested option constrained; nothing sent".into());
    }
    let value = number(option, SAMSUNG, "Value").ok_or("A/B capability value unavailable; nothing sent")?;
    // This experiment handles this known numeric vendor property only. Refuse richer options.
    if option.descendants().any(|n| n.has_tag_name((PSF, "ParameterRef"))) {
        return Err("A/B parameterized option unsupported; nothing sent".into());
    }
    for p in option.descendants().filter(|n| n.has_tag_name((PSF, "Property")) || n.has_tag_name((PSF, "ScoredProperty"))) {
        if !named(p, SAMSUNG, "Value") && !named(p, PSK, "DisplayName") {
            return Err("A/B capability has extra settings; nothing sent".into());
        }
    }
    if mode == Mode::Minimum {
        if selection(&base_doc)?.1 != value {
            return Err("A/B baseline Minimum value disagrees with capability; nothing sent".into());
        }
        return Ok(base.to_owned());
    }
    let property_kind = option
        .descendants()
        .find(|n| (n.has_tag_name((PSF, "Property")) || n.has_tag_name((PSF, "ScoredProperty"))) && named(*n, SAMSUNG, "Value"))
        .map(|n| n.tag_name().name())
        .ok_or("A/B capability property unavailable; nothing sent")?;
    let range = feature(&base_doc, SAMSUNG, "DocumentDarkenText")?.range();
    let replacement = format!(
        r#"<psf:Feature xmlns:psf="{PSF}" xmlns:s="{SAMSUNG}" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema" name="s:DocumentDarkenText"><psf:Option name="s:Off"><psf:{property_kind} name="s:Value"><psf:Value xsi:type="xsd:integer">{value}</psf:Value></psf:{property_kind}></psf:Option></psf:Feature>"#
    );
    let mut result = base.to_owned();
    result.replace_range(range, &replacement);
    Ok(result)
}
/// Called after driver validation, before a native print target exists. Rejection never prints.
pub fn verify_candidate(base: &str, requested: &str, returned: &str, mode: Mode) -> Result<(), String> {
    let result = (|| {
        let (base, request, returned) = (xml(base)?, xml(requested)?, xml(returned)?);
        core(&returned)?;
        if selection(&returned)? != selection(&request)? || selection(&returned)?.0 != mode {
            return Err("A/B DarkenText rejected or reverted; nothing sent".into());
        }
        if protected(&base)? != protected(&returned)? {
            return Err("A/B driver changed other job options; nothing sent".into());
        }
        Ok(())
    })();
    let observed = xml(returned).ok().and_then(|d| selection(&d).ok());
    audit(
        "ab.validation",
        format_args!(
            "requested={} observed={observed:?} confirmed_ticket={} status={} physical_effect=UNVERIFIED",
            mode.name(),
            result.is_ok(),
            if result.is_ok() { "TICKET_ONLY" } else { "REJECTED_OR_CHANGED_NOT_PRINTED" }
        ),
    )?;
    result
}
fn public_dm(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let dm = devmode(bytes).ok_or("A/B normalized DEVMODE unavailable; nothing sent")?;
    if dm.effective_dpi != Some(600)
        || dm.print_quality.is_some_and(|v| v != 600)
        || dm.y_resolution.is_some_and(|v| v != 600)
        || dm.scale_percent.is_some_and(|v| v != 100)
    {
        return Err("A/B normalized DPI/scale mismatch; nothing sent".into());
    }
    let size = u16::from_le_bytes(bytes.get(68..70).and_then(|s| s.try_into().ok()).ok_or("A/B header missing; nothing sent")?);
    bytes.get(72..usize::from(size)).map(Vec::from).ok_or_else(|| "A/B public DEVMODE missing; nothing sent".into())
}
fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100000001b3))
}
/// Read-only baseline normalization probe, labelled distinctly from the actual job.
pub fn remember_baseline(requested: &str, normalized: &str, dm: &[u8]) -> Result<(), String> {
    let doc = xml(normalized)?;
    core(&doc)?;
    if selection(&doc)?.0 != Mode::Minimum || selection(&doc)? != selection(&xml(requested)?)? {
        return Err("A/B normalized baseline Minimum is unconfirmed; nothing sent".into());
    }
    let protected = protected(&doc)?;
    let public_devmode = public_dm(dm)?;
    let context = capture().ok_or("A/B context unavailable; nothing sent")?;
    let mut state = context.ab_state.baseline.lock().map_err(|_| "A/B context unavailable; nothing sent")?;
    audit(
        "ab.baseline_probe",
        format_args!(
            "darken=Minimum protected_ticket_id={:016x} public_devmode_id={:016x} actual_job=false normalized_public_devmode={:?} dpi=600 physical_effect=UNVERIFIED",
            fingerprint(protected.as_bytes()),
            fingerprint(&public_devmode),
            devmode(dm)
        ),
    )?;
    *state = Some(Baseline { protected, public_devmode });
    Ok(())
}
/// Verify DEVMODE -> PrintTicket after the actual DocumentPropertiesW normalization.
pub fn verify_normalized(requested: &str, normalized: &str, dm: &[u8], mode: Mode) -> Result<(), String> {
    let result = (|| {
        let (request, doc) = (xml(requested)?, xml(normalized)?);
        core(&doc)?;
        if selection(&doc)? != selection(&request)? || selection(&doc)?.0 != mode {
            return Err("A/B normalized DarkenText rejected/reverted/unknown; nothing sent".into());
        }
        let context = capture().ok_or("A/B context unavailable; nothing sent")?;
        let state = context.ab_state.baseline.lock().map_err(|_| "A/B context unavailable; nothing sent")?;
        let baseline = state.as_ref().ok_or("A/B baseline probe missing; nothing sent")?;
        if protected(&doc)? != baseline.protected || public_dm(dm)? != baseline.public_devmode {
            return Err("A/B normalization changed other options; nothing sent".into());
        }
        Ok(())
    })();
    let normalized_doc = xml(normalized).ok();
    let observed = normalized_doc.as_ref().and_then(|d| selection(d).ok());
    let dpi = normalized_doc.as_ref().and_then(|d| feature(d, PSK, "PageResolution").ok());
    let dpi_x = dpi.and_then(|n| number(n, PSK, "ResolutionX"));
    let dpi_y = dpi.and_then(|n| number(n, PSK, "ResolutionY"));
    let scaling_none =
        normalized_doc.as_ref().and_then(|d| feature(d, PSK, "PageScaling").ok()).and_then(|n| choice(n).ok()).map(|n| n == format!("{{{PSK}}}None"));
    let media = normalized_doc.as_ref().and_then(|d| feature(d, PSK, "PageMediaSize").ok());
    let media_width = media.and_then(|n| number(n, PSK, "MediaSizeWidth"));
    let media_height = media.and_then(|n| number(n, PSK, "MediaSizeHeight"));
    let fine_edge_normal = normalized_doc
        .as_ref()
        .and_then(|d| feature(d, SAMSUNG, "DocumentFineEdge").ok())
        .and_then(|n| choice(n).ok())
        .map(|n| n == format!("{{{SAMSUNG}}}Normal"));
    audit(
        "ab.normalized",
        format_args!(
            "requested={} observed={observed:?} normalized_public_devmode={:?} confirmed_ticket={} status={} observed_dpi_x={dpi_x:?} observed_dpi_y={dpi_y:?} observed_scaling_none={scaling_none:?} observed_media_width={media_width:?} observed_media_height={media_height:?} observed_fine_edge_normal={fine_edge_normal:?} physical_effect=UNVERIFIED",
            mode.name(),
            devmode(dm),
            result.is_ok(),
            if result.is_ok() { "TICKET_ONLY" } else { "REJECTED_REVERTED_OR_UNKNOWN_NOT_PRINTED" }
        ),
    )?;
    result
}
/// Check the unchanged native calculation; never substitutes or raises DPI.
pub fn verify_raster_dpi(dpi: f32) -> Result<(), String> {
    audit("ab.rasterDPI", format_args!("actual_argument={dpi} required=600"))?;
    if dpi != 600.0 {
        return Err("A/B actual rasterDPI is not 600; nothing sent".into());
    }
    Ok(())
}

/// A/B-only allowlisted final-ticket observation; never logs option labels or arbitrary XML.
pub fn observe_validated_ticket(bytes: &[u8]) {
    let doc = std::str::from_utf8(bytes).ok().and_then(|s| xml(s).ok());
    let selected = doc.as_ref().and_then(|d| selection(d).ok());
    let dpi = doc.as_ref().and_then(|d| feature(d, PSK, "PageResolution").ok());
    let dpi_x = dpi.and_then(|n| number(n, PSK, "ResolutionX"));
    let dpi_y = dpi.and_then(|n| number(n, PSK, "ResolutionY"));
    let scaling_none =
        doc.as_ref().and_then(|d| feature(d, PSK, "PageScaling").ok()).and_then(|n| choice(n).ok()).map(|s| s == format!("{{{PSK}}}None"));
    crate::record(
        "native.ticket",
        format_args!(
            "scope=actual_validated_ticket darken={selected:?} observed_dpi_x={dpi_x:?} observed_dpi_y={dpi_y:?} observed_scaling_none={scaling_none:?} raw_option_labels_omitted=true"
        ),
    );
}

//! Read-only printer media capabilities. No queue/default setters or print target creation.
#[derive(Clone, Debug, PartialEq)]
pub struct Paper {
    pub name: String,
    pub size: (f64, f64),
}
#[derive(Clone, Debug, PartialEq)]
pub struct Capabilities {
    pub papers: Vec<Paper>,
    pub default: Option<usize>,
}
fn matches(a: (f64, f64), b: (f64, f64)) -> bool {
    let sorted = |(w, h): (f64, f64)| (w.min(h), w.max(h));
    let (a, b) = (sorted(a), sorted(b));
    (a.0 - b.0).abs() <= 0.5 && (a.1 - b.1).abs() <= 0.5
}
/// Prefer the existing selection if supported; otherwise a valid driver default/advertised paper.
/// The caller retains its desired paper for other printers and persistence.
pub fn resolve(wanted: &Paper, caps: &Capabilities) -> Result<Paper, String> {
    let valid =
        |p: &&Paper| p.size.0.is_finite() && p.size.1.is_finite() && p.size.0 > 0.0 && p.size.1 > 0.0 && p.size.0 <= 14400.0 && p.size.1 <= 14400.0;
    if caps.papers.iter().filter(valid).any(|p| matches(p.size, wanted.size)) {
        // Retain app's canonical dimensions, matching the same 0.5 pt tolerance as craft-print.
        return Ok(Paper { name: wanted.name.clone(), size: wanted.size });
    }
    caps.default
        .and_then(|i| caps.papers.get(i))
        .filter(|p| valid(p))
        .or_else(|| caps.papers.iter().find(valid))
        .cloned()
        .ok_or_else(|| "The printer has no readable supported paper; choose another printer.".into())
}
/// Factory GUI paper only. Headless, Fit/Actual and existing persisted selections are unchanged.
pub fn initial(windows: bool, locale: Option<&str>) -> usize {
    let korean = locale.and_then(|s| s.split(['-', '_', '.']).next()).is_some_and(|s| s.eq_ignore_ascii_case("ko"));
    if windows && korean { super::PAPERS.iter().position(|(n, _)| *n == "A4").unwrap_or(0) } else { 0 }
}
/// Uses the pinned safe native wrapper already used by craft-print. Query off the GUI thread.
pub fn capabilities(printer: &str) -> Result<Capabilities, String> {
    #[cfg(windows)]
    {
        use winprint::{
            printer::PrinterDevice,
            ticket::{FeatureOptionPack as _, PrintCapabilities, PrintTicket, PrintTicketBuilder},
        };
        let device = PrinterDevice::all().map_err(|e| e.to_string())?.into_iter().find(|d| d.name() == printer).ok_or("Printer unavailable")?;
        let caps = PrintCapabilities::fetch(&device).map_err(|e| e.to_string())?;
        let papers = caps
            .page_media_sizes()
            .take(1024)
            .map(|m| Paper {
                name: m.display_name().unwrap_or("Printer paper").to_string(),
                size: (f64::from(m.size().width_in_micron()) * 72.0 / 25400.0, f64::from(m.size().height_in_micron()) * 72.0 / 25400.0),
            })
            .collect::<Vec<_>>();
        let mut builder = PrintTicketBuilder::new(&device).map_err(|e| e.to_string())?;
        builder.merge(PrintTicket::default()).map_err(|e| e.to_string())?;
        let ticket = builder.build().map_err(|e| e.to_string())?;
        let default = default_size(ticket.get_xml()).and_then(|s| papers.iter().position(|p| matches(p.size, s)));
        Ok(Capabilities { papers, default })
    }
    #[cfg(not(windows))]
    {
        let _ = printer;
        Err("Native media discovery is Windows-only".into())
    }
}
#[cfg(any(windows, test))]
fn default_size(bytes: &[u8]) -> Option<(f64, f64)> {
    if bytes.len() > 4 * 1024 * 1024 {
        return None;
    }
    let doc = roxmltree::Document::parse_with_options(
        std::str::from_utf8(bytes).ok()?,
        roxmltree::ParsingOptions { nodes_limit: 100000, ..Default::default() },
    )
    .ok()?;
    const F: &str = "http://schemas.microsoft.com/windows/2003/08/printing/printschemaframework";
    const K: &str = "http://schemas.microsoft.com/windows/2003/08/printing/printschemakeywords";
    let named = |n: roxmltree::Node<'_, '_>, name: &str| {
        n.attribute("name").is_some_and(|v| {
            let (prefix, local) = v.split_once(':').unwrap_or(("", v));
            local == name && n.lookup_namespace_uri(if prefix.is_empty() { None } else { Some(prefix) }) == Some(K)
        })
    };
    let f = doc.root_element().children().find(|n| n.has_tag_name((F, "Feature")) && named(*n, "PageMediaSize"))?;
    let value = |name| {
        f.descendants()
            .find(|n| n.has_tag_name((F, "ScoredProperty")) && named(*n, name))?
            .children()
            .find(|n| n.has_tag_name((F, "Value")))?
            .text()?
            .parse::<f64>()
            .ok()
    };
    Some((value("MediaSizeWidth")? * 72.0 / 25400.0, value("MediaSizeHeight")? * 72.0 / 25400.0))
}
#[cfg(test)]
mod tests;

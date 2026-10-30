//! Windows compatibility at the spool boundary only. No full-page raster fallback.
//! Type 3 identifies a conservative compatibility class, not a diagnosed native fault.
use pdfcraft_cos::{Dict, Document, ObjRef, Object, SaveOptions, Stream};
use std::{collections::HashSet, sync::Arc};
mod outlines;
#[cfg(any(windows, test))]
mod worker;
const MAX_BYTES: usize = 256 * 1024 * 1024;
/// Inspect effective resources of final sheets (including inherited/nested forms).
pub fn has_type3(pdf: Arc<Vec<u8>>) -> Result<bool, String> {
    if pdf.len() > MAX_BYTES {
        return Err("Print compatibility byte limit".into());
    }
    type3_in(&Document::open(pdf).map_err(|e| e.to_string())?)
}
/// Convert only supported uncoloured outline glyphs. Ordinary PDFs are returned byte-for-byte.
/// Unsupported Type 3 content is refused; it is never silently flattened or sent unmodified.
pub fn prepare(pdf: Arc<Vec<u8>>) -> Result<Vec<u8>, String> {
    if !has_type3(pdf.clone())? {
        return Ok((*pdf).clone());
    }
    outlines::convert(pdf)
        .map_err(|e| format!("Type 3 compatibility could not preserve this PDF: {e}. Nothing sent; try explicit Print as image if appropriate."))
}
/// Same-binary entry; desktop and CLI call this before normal startup. No worker by default.
#[cfg(windows)]
pub fn worker_entry() -> Option<std::process::ExitCode> {
    worker::entry()
}
#[cfg(windows)]
pub(crate) fn submit(pdf: &[u8], job: &crate::spool::Job) -> Result<String, crate::PrintError> {
    if has_type3(Arc::new(pdf.to_vec())).map_err(crate::PrintError::Spool)? {
        return worker::submit(pdf, job).map_err(crate::PrintError::Spool);
    }
    craft_print::submit(pdf, job).map_err(|e| crate::PrintError::Spool(e.to_string()))
}
#[cfg(windows)]
pub(crate) fn submit_with_progress(pdf: &[u8], job: &crate::spool::Job, progress: &mut dyn FnMut(usize, usize)) -> Result<String, crate::PrintError> {
    if has_type3(Arc::new(pdf.to_vec())).map_err(crate::PrintError::Spool)? {
        return worker::submit(pdf, job).map_err(crate::PrintError::Spool);
    }
    craft_print::submit_with_progress(pdf, job, progress).map_err(|e| crate::PrintError::Spool(e.to_string()))
}
fn type3_in(doc: &Document) -> Result<bool, String> {
    fn walk(doc: &Document, obj: &Object, depth: usize, left: &mut usize, seen: &mut HashSet<ObjRef>) -> Result<bool, String> {
        if depth > 128 || *left == 0 {
            return Err("Print resource inspection exceeded its safety limit; nothing was sent to the printer".into());
        }
        *left -= 1;
        match obj {
            Object::Ref(r) if seen.insert(*r) => walk(doc, doc.try_get(r.num).map_err(|e| e.to_string())?.as_ref(), depth + 1, left, seen),
            Object::Array(items) => {
                for item in items {
                    if walk(doc, item, depth + 1, left, seen)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Object::Dict(d) | Object::Stream(Stream { dict: d, .. }) => {
                if d.get(b"Subtype").is_some_and(|o| doc.resolve(o).as_name() == Some(b"Type3")) {
                    return Ok(true);
                }
                for (_, value) in d.iter() {
                    if walk(doc, value, depth + 1, left, seen)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            _ => Ok(false),
        }
    }
    let pages = pdfcraft_model::pages(doc);
    if pages.is_empty() {
        return Err("The print-ready PDF has no readable sheets".into());
    }
    let (mut seen, mut left) = (HashSet::new(), 100_000);
    for page in pages {
        if let Some(resources) = page.dict.get(b"Resources")
            && walk(doc, resources, 0, &mut left, &mut seen)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests;

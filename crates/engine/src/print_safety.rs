//! Compatibility printing of already imposed sheets. Type 3 is a conservative routing
//! heuristic, not a diagnosis of a native Windows fault. Ordinary jobs keep their bytes.

use std::collections::HashSet;
use std::sync::Arc;

use pdfcraft_cos::{Dict, Document, ObjRef, Object, SaveOptions, Stream};
use pdfcraft_render::{PRINT_TAG, PageRenderer, RenderConfig, RenderRequest, Tile};

mod outlines;

/// Prepare only compatibility jobs; ordinary printing does not call this adapter.
/// Force-image remains available if a printer cannot consume the restricted outline result.
pub fn compatibility_pdf(pdf: Arc<Vec<u8>>, force_image: bool) -> Result<(Vec<u8>, String), String> {
    crate::guard(|| {
        if pdf.len() > MAX_BYTES {
            return Err("Compatibility input byte limit".into());
        }
        if !force_image && !has_type3(pdf.clone())? {
            return Ok(((*pdf).clone(), "original vector".into()));
        }
        if !force_image {
            match outlines::convert(pdf.clone()) {
                Ok(bytes) => return Ok((bytes, "Type 3 outlines to embedded Type 1C; original sheet geometry and other content retained".into())),
                Err(reason) => return image_pdf(pdf).map(|bytes| (bytes, format!("600 dpi image fallback: {reason}"))),
            }
        }
        image_pdf(pdf).map(|bytes| (bytes, "600 dpi image (explicit)".into()))
    })?
}

pub const IMAGE_DPI: u32 = 600;
const MAX_BYTES: usize = 256 * 1024 * 1024;
const MAX_JOB_PIXELS: u64 = 500_000_000;
const MAX_PAGE_PIXELS: u64 = 100_000_000;
const TILE_SIDE: u32 = 2048;

/// Inspect effective page resources, including inherited resources and nested Form XObjects.
/// Do not search raw bytes: compressed fonts and unrelated strings must be handled correctly.
pub fn has_type3(pdf: Arc<Vec<u8>>) -> Result<bool, String> {
    let doc = Document::open(pdf).map_err(|e| e.to_string())?;
    type3_in(&doc)
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

fn raster_size(w: f64, h: f64) -> Result<(u32, u32), String> {
    let (x, y) = ((w * f64::from(IMAGE_DPI) / 72.0).ceil(), (h * f64::from(IMAGE_DPI) / 72.0).ceil());
    if !x.is_finite() || !y.is_finite() || x < 1.0 || y < 1.0 || x > 100_000.0 || y > 100_000.0 || x * y > MAX_PAGE_PIXELS as f64 {
        return Err("Sheet exceeds the 600 dpi image-print safety limit; use a smaller paper or page range".into());
    }
    Ok((x as u32, y as u32))
}

/// Flatten final sheets at 600 dpi, losslessly, in bounded tiles. Imposition/annotations,
/// selected page order and physical sheet sizes have already been applied by `print_pdf`.
/// Renderer errors/truncation abort the whole job before submission; never print partial pages.
pub fn image_pdf(pdf: Arc<Vec<u8>>) -> Result<Vec<u8>, String> {
    crate::guard(|| image_pdf_inner(pdf))?
}

fn image_pdf_inner(pdf: Arc<Vec<u8>>) -> Result<Vec<u8>, String> {
    if pdf.len() > MAX_BYTES {
        return Err("Print-ready PDF exceeds the image-print input limit".into());
    }
    let source = Document::open(pdf.clone()).map_err(|e| e.to_string())?;
    let pages = pdfcraft_model::pages(&source);
    if pages.is_empty() || pages.len() > 1000 {
        return Err("Invalid image-print sheet count".into());
    }
    let mut total = 0u64;
    let sizes = pages
        .iter()
        .map(|p| {
            let size = p.display_size(&source);
            let (w, h) = raster_size(size.0, size.1)?;
            total = total.saturating_add(u64::from(w) * u64::from(h));
            if total > MAX_JOB_PIXELS {
                return Err("Image-print job exceeds its pixel budget; print a smaller page range".into());
            }
            Ok((size, w, h))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut renderer = PageRenderer::new(pdf, RenderConfig { reject_oversize: true, ..Default::default() });
    if renderer.page_count() != pages.len() {
        return Err("Image renderer and print-ready sheet counts disagree".into());
    }
    let mut out = Document::new_empty();
    let tree = out.root().and_then(|r| out.get(r).as_dict().and_then(|d| d.reference(b"Pages"))).ok_or("New image PDF has no page tree")?;
    let mut kids = Vec::new();
    let mut compressed = 0usize;
    for (page, ((width_pt, height_pt), width, height)) in sizes.into_iter().enumerate() {
        let mut images = Dict::new();
        let mut commands = Vec::new();
        let mut index = 0usize;
        for y in (0..height).step_by(TILE_SIDE as usize) {
            for x in (0..width).step_by(TILE_SIDE as usize) {
                let (w, h) = ((width - x).min(TILE_SIDE), (height - y).min(TILE_SIDE));
                let rendered = renderer.render(RenderRequest {
                    page,
                    scale: IMAGE_DPI as f32 / 72.0,
                    tile: Some(Tile { x, y, w, h }),
                    tag: PRINT_TAG,
                    ..Default::default()
                });
                if let Some(error) = rendered.error {
                    return Err(format!("Image-print sheet {}: {error}", page + 1));
                }
                if !rendered.warnings.is_empty() || (rendered.width, rendered.height) != (w, h) || rendered.rgba.len() != w as usize * h as usize * 4
                {
                    return Err(format!("Image-print sheet {} was incomplete; nothing was sent", page + 1));
                }
                let rgb: Vec<u8> = rendered
                    .rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .flat_map(|p| {
                        // Premultiplied RGBA over white, without rounding a transparent edge dark.
                        let white = 255 - p[3];
                        [p[0].saturating_add(white), p[1].saturating_add(white), p[2].saturating_add(white)]
                    })
                    .collect();
                let mut d = Dict::new();
                d.set(b"Type".to_vec(), Object::name("XObject"));
                d.set(b"Subtype".to_vec(), Object::name("Image"));
                d.set(b"Width".to_vec(), i64::from(w));
                d.set(b"Height".to_vec(), i64::from(h));
                d.set(b"BitsPerComponent".to_vec(), 8i64);
                d.set(b"ColorSpace".to_vec(), Object::name("DeviceRGB"));
                d.set(b"Interpolate".to_vec(), Object::Bool(false));
                let stream = Stream::flate(d, &rgb);
                compressed = compressed.saturating_add(stream.raw.len());
                if compressed > MAX_BYTES {
                    return Err("Image-print output exceeds its byte budget; print a smaller page range".into());
                }
                let image = out.add(Object::Stream(stream));
                let name = format!("Im{index}");
                images.set(name.as_bytes().to_vec(), Object::Ref(image));
                // Shared pixel-grid boundaries avoid gaps. Keep the exact original sheet size;
                // rounding the raster dimensions changes sampling by less than one pixel only.
                let (sx, sy) = (width_pt / f64::from(width), height_pt / f64::from(height));
                commands.extend_from_slice(
                    format!(
                        "q {} 0 0 {} {} {} cm /{name} Do Q\n",
                        f64::from(w) * sx,
                        f64::from(h) * sy,
                        f64::from(x) * sx,
                        height_pt - f64::from(y + h) * sy
                    )
                    .as_bytes(),
                );
                index += 1;
            }
        }
        let content = out.add(Object::Stream(Stream::flate(Dict::new(), &commands)));
        let mut resources = Dict::new();
        resources.set(b"XObject".to_vec(), Object::Dict(images));
        let mut d = Dict::new();
        d.set(b"Type".to_vec(), Object::name("Page"));
        d.set(b"Parent".to_vec(), Object::Ref(tree));
        d.set(b"MediaBox".to_vec(), Object::Array(vec![0.into(), 0.into(), Object::Real(width_pt), Object::Real(height_pt)]));
        d.set(b"Resources".to_vec(), Object::Dict(resources));
        d.set(b"Contents".to_vec(), Object::Ref(content));
        kids.push(Object::Ref(out.add(d)));
    }
    out.update_dict(tree, |d| {
        d.set(b"Count".to_vec(), kids.len() as i64);
        d.set(b"Kids".to_vec(), Object::Array(kids));
    })
    .map_err(|e| e.to_string())?;
    let bytes = pdfcraft_cos::write_full(&out, &SaveOptions::default()).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("Image-print PDF exceeds the output limit".into());
    }
    Ok(bytes)
}

/// Ordinary native jobs retain the old path. On Windows only suspected/explicit image jobs
/// run in the same application's opt-in worker process, so a driver access violation cannot
/// close the editor. Never retry a failed submission: sheets might already have been printed.
pub fn submit(pdf: Vec<u8>, job: &crate::print::spool::Job, force_image: bool) -> Result<String, String> {
    #[cfg(windows)]
    {
        let pdf = Arc::new(pdf);
        if force_image || has_type3(pdf.clone())? {
            return worker::submit(&pdf, job, force_image);
        }
        crate::print::spool::submit(&pdf, job).map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let bytes = if force_image { image_pdf(Arc::new(pdf))? } else { pdf };
        crate::print::spool::submit(&bytes, job).map_err(|e| e.to_string())
    }
}

#[cfg(any(windows, test))]
mod worker;

/// Called before GUI/single-instance/CLI initialization. `None` for every normal launch.
pub fn worker_entry() -> Option<std::process::ExitCode> {
    #[cfg(windows)]
    {
        worker::entry()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(test)]
mod tests;

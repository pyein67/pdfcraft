//! Restricted Type 3 -> embedded Type 1C conversion (ISO 32000, CFF/Type 2 formats).
//! Only uncoloured, single nonzero fills of paths are accepted. Never approximate an
//! unsupported paint: the caller refuses the compatibility job before native submission.

use super::{Arc, Dict, Document, HashSet, MAX_BYTES, ObjRef, Object, SaveOptions, Stream, type3_in};
use pdfcraft_content::Matrix;

type Checked<T> = Result<T, &'static str>;
const LIMIT: usize = 16 * 1024 * 1024;

fn numbers<const N: usize>(o: &Object) -> Checked<[f64; N]> {
    let a = o.as_array().ok_or("expected numeric array")?;
    if a.len() != N {
        return Err("wrong numeric array length");
    }
    let mut v = [0.0; N];
    for (x, o) in v.iter_mut().zip(a) {
        *x = o.as_f64().filter(|x| x.is_finite()).ok_or("nonfinite number")?;
    }
    Ok(v)
}

/// CFF DICT long integer. Offsets have fixed length, allowing a single layout pass.
fn integer(n: i32, out: &mut Vec<u8>) {
    out.push(29);
    out.extend_from_slice(&n.to_be_bytes());
}
fn index(items: &[Vec<u8>]) -> Checked<Vec<u8>> {
    let count = u16::try_from(items.len()).map_err(|_| "too many CFF entries")?;
    let mut out = count.to_be_bytes().to_vec();
    if items.is_empty() {
        return Ok(out);
    }
    out.push(4);
    let mut offset = 1u32;
    out.extend_from_slice(&offset.to_be_bytes());
    for item in items {
        offset = offset.checked_add(u32::try_from(item.len()).map_err(|_| "CFF entry too large")?).ok_or("CFF overflow")?;
        if offset as usize > LIMIT {
            return Err("CFF byte limit");
        }
        out.extend_from_slice(&offset.to_be_bytes());
    }
    for item in items {
        out.extend_from_slice(item);
    }
    Ok(out)
}

/// Type 2 16.16 operands keep subpixel outline precision without integer rounding of stems.
fn operand(n: f64, out: &mut Vec<u8>) -> Checked<()> {
    if !n.is_finite() || n.abs() > 32000.0 {
        return Err("outline coordinate limit");
    }
    out.push(255);
    out.extend_from_slice(&((n * 65536.0).round() as i32).to_be_bytes());
    Ok(())
}

#[derive(Default)]
struct Path {
    bytes: Vec<u8>,
    at: (f64, f64),
    start: Option<(f64, f64)>,
    bounds: Option<[f64; 4]>,
}
impl Path {
    fn point(&mut self, p: (f64, f64)) -> Checked<()> {
        if !p.0.is_finite() || !p.1.is_finite() || p.0.abs() > 16000.0 || p.1.abs() > 16000.0 {
            return Err("absolute outline coordinate limit");
        }
        // Quantize absolute points before taking relative deltas, avoiding accumulated drift.
        let p = ((p.0 * 65536.0).round() / 65536.0, (p.1 * 65536.0).round() / 65536.0);
        operand(p.0 - self.at.0, &mut self.bytes)?;
        operand(p.1 - self.at.1, &mut self.bytes)?;
        self.at = p;
        self.bounds = Some(match self.bounds {
            None => [p.0, p.1, p.0, p.1],
            Some(b) => [b[0].min(p.0), b[1].min(p.1), b[2].max(p.0), b[3].max(p.1)],
        });
        Ok(())
    }
    fn close(&mut self) -> Checked<()> {
        if let Some(p) = self.start.take() {
            self.point(p)?;
            self.bytes.push(5);
        }
        Ok(())
    }
    fn moveto(&mut self, p: (f64, f64)) -> Checked<()> {
        self.close()?;
        self.point(p)?;
        self.bytes.push(21);
        self.start = Some(p);
        Ok(())
    }
}

pub(super) fn glyph(data: &[u8], matrix: Matrix, width: f64) -> Checked<(Vec<u8>, Option<[f64; 4]>)> {
    let parsed = pdfcraft_content::parse(data);
    if parsed.skipped != 0 || parsed.ops.len() > 100_000 {
        return Err("unreadable or over-limit glyph");
    }
    let mut path = Path::default();
    operand(width, &mut path.bytes)?;
    let mut ctm = Matrix::IDENTITY;
    let (mut declared, mut painted, mut began) = (false, false, false);
    let mut declared_bounds = [0.0; 4];
    for op in &parsed.ops {
        if painted {
            return Err("multiple glyph paints");
        }
        let n = |i| op.num(i).filter(|x| x.is_finite()).ok_or("invalid glyph operand");
        let point = |x, y| {
            let (x, y) = ctm.then(&matrix).apply(x, y);
            (x * 1000.0, y * 1000.0)
        };
        match op.op.as_slice() {
            b"d1" if !declared && !began && op.operands.len() == 6 => {
                for i in 0..6 {
                    n(i)?;
                }
                if (n(0)? * matrix.0[0] * 1000.0 - width).abs() > 0.001 {
                    return Err("inconsistent glyph width");
                }
                declared_bounds = matrix.bbox([n(2)?, n(3)?, n(4)?, n(5)?]).map(|v| v * 1000.0);
                if declared_bounds.iter().any(|v| !v.is_finite()) {
                    return Err("invalid glyph bounds");
                }
                if n(1)? != 0.0 {
                    return Err("vertical glyph advance");
                }
                declared = true;
            }
            b"cm" if declared && !began => {
                ctm = Matrix(numbers::<6>(&Object::Array(op.operands.clone()))?).then(&ctm);
            }
            b"m" if declared && op.operands.len() == 2 => {
                path.moveto(point(n(0)?, n(1)?))?;
                began = true;
            }
            b"l" if path.start.is_some() && op.operands.len() == 2 => {
                path.point(point(n(0)?, n(1)?))?;
                path.bytes.push(5);
            }
            b"c" if path.start.is_some() && op.operands.len() == 6 => {
                for i in [0, 2, 4] {
                    path.point(point(n(i)?, n(i + 1)?))?;
                }
                path.bytes.push(8);
            }
            b"h" if op.operands.is_empty() => {
                path.close()?;
            }
            b"f" | b"F" if declared && began && op.operands.is_empty() => {
                path.close()?;
                painted = true;
            }
            _ => return Err("glyph needs unsupported paint or graphics state"),
        }
        if path.bytes.len() > 65_534 {
            return Err("glyph byte limit");
        }
    }
    if !declared || (began && !painted) {
        return Err("incomplete glyph");
    }
    if let Some(b) = path.bounds {
        let d = declared_bounds;
        if b[0] < d[0] - 0.001 || b[1] < d[1] - 0.001 || b[2] > d[2] + 0.001 || b[3] > d[3] + 0.001 {
            return Err("outline exceeds glyph bounds");
        }
    }
    // Width plus endchar handles an empty glyph; otherwise the first rmoveto consumes width.
    path.bytes.push(14);
    Ok((path.bytes, path.bounds))
}

fn cff(name: &str, glyphs: &[Vec<u8>], glyph_names: &[Vec<u8>], bounds: [f64; 4]) -> Checked<Vec<u8>> {
    let names = index(&[name.as_bytes().to_vec()])?;
    let strings = index(glyph_names)?;
    let chars = index(glyphs)?;
    let top = |charset: i32, charstrings: i32, private: i32| {
        let mut d = Vec::new();
        for (i, b) in bounds.into_iter().enumerate() {
            integer(if i < 2 { b.floor() as i32 } else { b.ceil() as i32 }, &mut d);
        }
        d.push(5);
        integer(charset, &mut d);
        d.push(15);
        integer(charstrings, &mut d);
        d.push(17);
        integer(4, &mut d);
        integer(private, &mut d);
        d.push(18);
        d
    };
    let top_size = index(&[top(0, 0, 0)])?.len();
    let charset_offset = 4 + names.len() + top_size + strings.len() + 2;
    let mut charset = vec![0];
    for i in 0..glyphs.len().saturating_sub(1) {
        charset.extend_from_slice(&(391 + i as u16).to_be_bytes());
    }
    let char_offset = charset_offset + charset.len();
    let dict = index(&[top(charset_offset as i32, char_offset as i32, (char_offset + chars.len()) as i32)])?;
    let mut out = vec![1, 0, 4, 4];
    for part in [names, dict, strings, vec![0, 0], charset, chars, vec![139, 20, 139, 21]] {
        out.extend_from_slice(&part);
    }
    if out.len() > LIMIT {
        return Err("font byte limit");
    }
    Ok(out)
}

fn convert_font(doc: &mut Document, r: ObjRef, font: &Dict, budget: &mut usize) -> Checked<()> {
    let matrix = Matrix(numbers::<6>(&doc.resolve(font.get(b"FontMatrix").ok_or("missing font matrix")?))?);
    if matrix.0[1] != 0.0 || matrix.0[0] <= 0.0 {
        return Err("nonhorizontal font advances");
    }
    if font.get(b"Resources").is_some_and(|r| doc.dict(r).is_none_or(|d| d.iter().next().is_some())) {
        return Err("glyph resources cannot be preserved by Type 1C conversion");
    }
    let first = font.get(b"FirstChar").and_then(Object::as_int).ok_or("missing first code")?;
    let last = font.get(b"LastChar").and_then(Object::as_int).ok_or("missing last code")?;
    if first < 0 || last < first || last > 255 {
        return Err("invalid code range");
    }
    let widths = doc.resolve(font.get(b"Widths").ok_or("missing widths")?);
    let widths = widths.as_array().filter(|a| a.len() == (last - first + 1) as usize).ok_or("invalid widths")?;
    let encoding = doc.dict(font.get(b"Encoding").ok_or("missing encoding")?).ok_or("unsupported encoding")?;
    if encoding.get(b"BaseEncoding").is_some() {
        return Err("implicit encoding");
    }
    let differences = doc.resolve(encoding.get(b"Differences").ok_or("missing differences")?);
    let differences = differences.as_array().ok_or("invalid differences")?;
    let mut codes = vec![None; 256];
    let mut code = 256usize;
    for item in differences {
        match item {
            Object::Int(i) if (0..=255).contains(i) => code = *i as usize,
            Object::Name(n) => {
                *codes.get_mut(code).ok_or("encoding overflow")? = Some(n.clone());
                code += 1;
            }
            _ => return Err("invalid encoding item"),
        }
    }
    let procs = doc.dict(font.get(b"CharProcs").ok_or("missing glyph procedures")?).ok_or("invalid glyph procedures")?;
    for (code, name) in codes.iter().enumerate() {
        if name.is_some() && !(first as usize..=last as usize).contains(&code) {
            return Err("encoding outside explicit widths");
        }
    }
    let mut glyphs = vec![vec![14]];
    let mut glyph_names = Vec::new();
    let mut used_names = HashSet::new();
    let mut represented_names = HashSet::new();
    let mut out_widths = Vec::new();
    let mut bounds = [0.0f64; 4];
    for code in first..=last {
        let glyph_name = codes.get(code as usize).and_then(Option::as_ref).ok_or("unmapped code")?;
        represented_names.insert(glyph_name.clone());
        let proc = doc.resolve(procs.get(glyph_name).ok_or("missing glyph procedure")?);
        let Object::Stream(proc) = proc.as_ref() else {
            return Err("invalid glyph procedure");
        };
        let data = proc.decoded_within((*budget).min(LIMIT)).map_err(|_| "glyph decode failed")?;
        *budget = budget.checked_sub(data.len()).ok_or("font decode budget")?;
        let width = widths.get((code - first) as usize).and_then(Object::as_f64).ok_or("invalid width")? * matrix.0[0] * 1000.0;
        let (program, b) = glyph(&data, matrix, width)?;
        if let Some(b) = b {
            bounds = [bounds[0].min(b[0]), bounds[1].min(b[1]), bounds[2].max(b[2]), bounds[3].max(b[3])];
        }
        if glyph_name.as_slice() == b".notdef" {
            if b.is_some() {
                return Err("painted notdef glyph");
            }
        } else {
            if glyph_name.is_empty()
                || glyph_name.len() > 127
                || !glyph_name.iter().all(|c| c.is_ascii_alphanumeric() || b"._-".contains(c))
                || !used_names.insert(glyph_name.clone())
            {
                return Err("unsupported glyph name or alias");
            }
            glyph_names.push(glyph_name.clone());
            glyphs.push(program);
        }
        out_widths.push(Object::Real(width));
    }
    if procs.iter().any(|(name, _)| !represented_names.contains(name)) {
        return Err("unmapped glyph procedures cannot be preserved by Type 1C conversion");
    }
    let name = format!("PdfCraftPrintSafe{}", r.num);
    let mut stream = Dict::new();
    stream.set(b"Subtype".to_vec(), Object::name("Type1C"));
    let program = cff(&name, &glyphs, &glyph_names, bounds)?;
    let program = doc.add(Object::Stream(Stream::flate(stream, &program)));
    let mut descriptor = Dict::new();
    descriptor.set(b"Type".to_vec(), Object::name("FontDescriptor"));
    descriptor.set(b"FontName".to_vec(), Object::name(&name));
    descriptor.set(b"Flags".to_vec(), 4i64);
    descriptor.set(b"FontBBox".to_vec(), Object::Array(bounds.map(Object::Real).to_vec()));
    descriptor.set(b"ItalicAngle".to_vec(), 0i64);
    descriptor.set(b"Ascent".to_vec(), Object::Real(bounds[3]));
    descriptor.set(b"Descent".to_vec(), Object::Real(bounds[1]));
    descriptor.set(b"CapHeight".to_vec(), Object::Real(bounds[3]));
    descriptor.set(b"StemV".to_vec(), 0i64);
    descriptor.set(b"FontFile3".to_vec(), Object::Ref(program));
    let descriptor = doc.add(descriptor);
    let mut converted = Dict::new();
    converted.set(b"Type".to_vec(), Object::name("Font"));
    converted.set(b"Subtype".to_vec(), Object::name("Type1"));
    converted.set(b"BaseFont".to_vec(), Object::name(&name));
    converted.set(b"FirstChar".to_vec(), first);
    converted.set(b"LastChar".to_vec(), last);
    converted.set(b"Widths".to_vec(), Object::Array(out_widths));
    converted.set(b"FontDescriptor".to_vec(), Object::Ref(descriptor));
    converted.set(b"Encoding".to_vec(), Object::Dict(encoding));
    if let Some(unicode) = font.get(b"ToUnicode") {
        converted.set(b"ToUnicode".to_vec(), unicode.clone());
    }
    doc.set(r, converted);
    Ok(())
}

pub(super) fn convert(pdf: Arc<Vec<u8>>) -> Result<Vec<u8>, String> {
    let mut doc = Document::open(pdf).map_err(|e| e.to_string())?;
    let mut fonts = Vec::new();
    let mut streams = Vec::new();
    // Checking text paint state globally is deliberately conservative: Type 3 ignores Tr
    // stroke/clip modes, whereas Type 1 would obey them. Preserve semantics by falling back.
    for page in pdfcraft_model::pages(&doc) {
        if let Some(c) = page.dict.get(b"Contents") {
            match doc.resolve(c).as_ref() {
                Object::Array(a) => streams.extend(a.iter().cloned()),
                _ => streams.push(c.clone()),
            }
        }
    }
    for (i, (r, o)) in doc.scan_objects_checked().enumerate() {
        if i >= 100_000 {
            return Err("outline object limit".into());
        }
        let o = o.map_err(|e| e.to_string())?;
        if let Some(d) = o.as_dict() {
            if d.name(b"Subtype") == Some(b"Type3") {
                fonts.push((r, d.clone()));
                if fonts.len() > 32 {
                    return Err("outline font limit".into());
                }
            } else if d.name(b"Subtype") == Some(b"Form") {
                streams.push(Object::Ref(r));
            }
        }
    }
    let mut budget = LIMIT;
    for stream in streams {
        let stream = doc.resolve(&stream);
        let Object::Stream(s) = stream.as_ref() else {
            return Err("invalid page content".into());
        };
        let data = s.decoded_within(budget).map_err(|e| e.to_string())?;
        budget = budget.checked_sub(data.len()).ok_or("outline content budget")?;
        let p = pdfcraft_content::parse(&data);
        if p.skipped != 0 || p.ops.iter().any(|o| o.is("Tr") && o.num(0) != Some(0.0)) {
            return Err("text paint mode cannot be preserved by Type 1C conversion".into());
        }
    }
    if fonts.is_empty() {
        return Err("no indirect outline font".into());
    }
    for (r, font) in fonts {
        convert_font(&mut doc, r, &font, &mut budget).map_err(str::to_owned)?;
    }
    if type3_in(&doc)? {
        return Err("remaining Type 3 resources cannot be preserved by Type 1C conversion".into());
    }
    let out = pdfcraft_cos::write_full(&doc, &SaveOptions::default()).map_err(|e| e.to_string())?;
    if out.len() > MAX_BYTES {
        return Err("outline output byte limit".into());
    }
    Ok(out)
}

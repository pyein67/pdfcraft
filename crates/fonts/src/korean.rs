//! Korean build-input selection and the shared Unicode PDF font writer.
//! No system fonts are copied into documents. The full, licensed TrueType program is retained.

use crate::{CraftFont, embedding, pdf::Metrics};
use pdfcraft_cos::{Dict, Document};

pub fn is_hangul(c: char) -> bool {
    matches!(u32::from(c), 0x1100..=0x11ff | 0x3130..=0x318f | 0xa960..=0xa97f | 0xac00..=0xd7ff)
}

pub fn contains_hangul(text: &str) -> bool {
    text.chars().any(is_hangul)
}

pub fn ui_fonts() -> Vec<&'static CraftFont> {
    crate::CRAFT_FONTS.iter().filter(|f| f.covers("Hang")).collect()
}

pub fn document_font() -> Option<&'static CraftFont> {
    ui_fonts().into_iter().find(|f| f.style == "Regular")
}

/// A Korean row uses one face for Latin and Hangul, so its advances and baseline agree.
pub fn can_draw(text: &str) -> bool {
    document_font().is_some_and(|f| embedding::covers(f.bytes, &compose(text)))
}

/// Compose modern Hangul Jamo emitted by an IME or decomposed clipboard text. Leave all other
/// characters intact. Unicode §3.12's algorithm needs no locale or compatibility folding.
pub fn compose(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let u = u32::from(c);
        let mut syllable = if (0x1100..0x1113).contains(&u) {
            match chars.peek().copied().map(u32::from) {
                Some(v @ 0x1161..=0x1175) => {
                    chars.next();
                    0xac00 + (u - 0x1100) * 588 + (v - 0x1161) * 28
                }
                _ => u,
            }
        } else {
            u
        };
        if (0xac00..=0xd7a3).contains(&syllable)
            && (syllable - 0xac00).is_multiple_of(28)
            && let Some(t @ 0x11a8..=0x11c2) = chars.peek().copied().map(u32::from)
        {
            chars.next();
            syllable += t - 0x11a7;
        }
        // Composition only produces a valid Hangul syllable, otherwise it preserves `c`.
        out.push(char::from_u32(syllable).unwrap_or(c));
    }
    out
}

/// Canonically decompose Hangul for matching across composed and decomposed PDF glyphs,
/// retaining the caller's glyph index for every emitted character.
pub fn search_chars(text: &str) -> Vec<char> {
    let mut out = Vec::new();
    for c in text.chars() {
        let u = u32::from(c);
        if (0xac00..=0xd7a3).contains(&u) {
            let s = u - 0xac00;
            out.extend(char::from_u32(0x1100 + s / 588));
            out.extend(char::from_u32(0x1161 + (s % 588) / 28));
            if !s.is_multiple_of(28) {
                out.extend(char::from_u32(0x11a7 + s % 28));
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Advance from the exact face used by the PDF writer, in points.
pub fn text_width(text: &str, size: f64) -> Option<f64> {
    use skrifa::{
        FontRef, MetadataProvider,
        instance::{LocationRef, Size},
    };
    let face = FontRef::new(document_font()?.bytes).ok()?;
    let metrics = face.glyph_metrics(Size::unscaled(), LocationRef::default());
    let em = f64::from(face.metrics(Size::unscaled(), LocationRef::default()).units_per_em);
    if em <= 0.0 {
        return None;
    }
    compose(text).chars().try_fold(0.0, |width, c| {
        let gid = face.charmap().map(c)?;
        Some(width + f64::from(metrics.advance_width(gid)?) / em * size)
    })
}

pub fn embed(doc: &mut Document, fonts: &mut Dict) -> Result<(String, Metrics), String> {
    let face = document_font().ok_or("this build has no Korean font; run cargo xtask korean-fonts <external directory>, then build with CRAFT_FONTS_DIR and PDFCRAFT_KOREAN_REQUIRED=1")?;
    let name = embedding::embed(doc, fonts, face.bytes, face.family)?;
    let font = fonts.get(name.as_bytes()).and_then(|o| doc.dict(o)).ok_or("embedded Korean font is missing")?;
    Ok((name, Metrics::from_dict(doc, &font)))
}

pub fn width(metrics: &Metrics, text: &str, size: f64) -> f64 {
    metrics.encode(&compose(text)).map(|bytes| metrics.codes(&bytes).iter().map(|(code, _)| metrics.width(*code) * size).sum()).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn korean_build_face_covers_modern_syllables_and_allows_embedding() {
        let Some(face) = document_font() else {
            assert!(std::env::var_os("PDFCRAFT_KOREAN_REQUIRED").is_none());
            return;
        };
        let syllables: String = (0xac00..=0xd7a3).filter_map(char::from_u32).collect();
        assert!(embedding::covers(face.bytes, &syllables));
        // Curated build inputs have their own hash-verified OFL file (xtask korean-fonts).
        // validate_document_input additionally requires a licence description inside an
        // imported font's name table, which this unmodified 2010 Nanum face does not carry.
        embedding::validate(face.bytes).expect("font permissions allow editable outline embedding");
    }

    #[test]
    fn modern_hangul_composition_is_total_and_preserves_other_scripts() {
        assert_eq!(compose("한글 ABC 한글 ㄱㄴ"), "한글 ABC 한글 ㄱㄴ");
        assert_eq!(compose("각"), "각");
        assert_eq!(compose("ᄀᄀ ᅡᆨ"), "ᄀᄀ ᅡᆨ");
        assert_eq!(compose(""), "");
        for cp in 0xac00..=0xd7a3 {
            let s = char::from_u32(cp).unwrap().to_string();
            assert_eq!(compose(&search_chars(&s).into_iter().collect::<String>()), s);
        }
    }
}

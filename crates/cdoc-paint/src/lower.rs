//! The layout IR lowered to display-list operations. Text needs no translation: the layout's
//! glyph runs are already shaped, so each becomes a `GLYPH_RUN` with the same glyph ids,
//! advances, offsets and clusters, and the face entry collects the glyphs its runs use with
//! their `hmtx` advances (what a glyph atlas prepares).

use crate::op::{
    DisplayList, DocumentInfo, FaceEntry, Glyph, GlyphMetric, Op, PageOps, Rgba, RunHead,
};
use crate::PaintError;
use cdoc_layout::{Draw, FontPort, GlyphRun, Layout, Page};

/// One face: parsed, with the glyph table being collected.
struct FaceBuild<'a> {
    face: ttf_parser::Face<'a>,
    /// Glyph id → whether it is in `glyphs` yet.
    seen: Vec<bool>,
    glyphs: Vec<GlyphMetric>,
}

impl FaceBuild<'_> {
    fn register(&mut self, id: u16) {
        let at = usize::from(id);
        if at >= self.seen.len() {
            self.seen.resize(at + 1, false);
        }
        if !std::mem::replace(&mut self.seen[at], true) {
            let advance = self
                .face
                .glyph_hor_advance(ttf_parser::GlyphId(id))
                .unwrap_or(0);
            self.glyphs.push(GlyphMetric { id, advance });
        }
    }
}

fn parse_faces<'a>(
    layout: &Layout,
    port: &'a dyn FontPort,
) -> Result<Vec<FaceBuild<'a>>, PaintError> {
    layout
        .faces
        .iter()
        .map(|key| {
            let source = port.face(&key.family, key.bold, key.italic)?;
            let face = ttf_parser::Face::parse(source.bytes, 0)
                .map_err(|_| PaintError::Unreadable(key.clone()))?;
            Ok(FaceBuild {
                seen: vec![false; usize::from(face.number_of_glyphs())],
                face,
                glyphs: Vec::new(),
            })
        })
        .collect()
}

fn glyph_run(run: &GlyphRun, faces: &mut [FaceBuild<'_>]) -> Result<Op, PaintError> {
    let face = faces
        .get_mut(usize::from(run.face.0))
        .ok_or(PaintError::NoFace(run.face.0))?;
    let glyphs = run
        .glyphs
        .iter()
        .map(|g| {
            face.register(g.id);
            Glyph {
                id: g.id,
                cluster: g.cluster,
                advance: g.advance,
                x_offset: g.x_offset,
                y_offset: g.y_offset,
            }
        })
        .collect();
    Ok(Op::GlyphRun {
        head: RunHead {
            face: run.face,
            size: run.size,
            colour: Rgba::opaque(run.colour),
            x: run.x,
            y: run.y,
        },
        text: run.text.clone(),
        glyphs,
    })
}

fn draw(d: &Draw, faces: &mut [FaceBuild<'_>]) -> Result<Op, PaintError> {
    let opaque = |c: &cdoc_model::Color| Rgba::opaque(*c);
    Ok(match d {
        Draw::Text(run) => glyph_run(run, faces)?,
        Draw::Rule {
            from,
            to,
            width,
            colour,
        } => Op::Line {
            from: *from,
            to: *to,
            width: *width,
            colour: opaque(colour),
        },
        Draw::Fill { rect, colour } => Op::FillRect {
            rect: *rect,
            colour: opaque(colour),
        },
        Draw::Path { segments, colour } => Op::FillPath {
            segments: segments.clone(),
            colour: opaque(colour),
        },
        Draw::Stroke {
            segments,
            width,
            colour,
        } => Op::StrokePath {
            segments: segments.clone(),
            width: *width,
            colour: opaque(colour),
        },
        Draw::Image { rect, content_id } => Op::Image {
            rect: *rect,
            content_id: content_id.as_str().to_owned(),
        },
    })
}

fn page(
    number: usize,
    layout: &Layout,
    page: &Page,
    faces: &mut [FaceBuild<'_>],
) -> Result<PageOps, PaintError> {
    let number = u32::try_from(number).map_err(|_| PaintError::TooLarge)?;
    let mut ops = Vec::with_capacity(page.draws.len() + page.links.len());
    for d in &page.draws {
        ops.push(draw(d, faces)?);
    }
    ops.extend(page.links.iter().map(|link| Op::Link {
        rect: link.rect,
        href: link.href.clone(),
    }));
    let fields = layout.signatures.iter().filter(|s| s.page == number);
    ops.extend(fields.map(|s| Op::SigField {
        rect: s.rect,
        id: s.id.as_str().to_owned(),
        role: s.signer_role.clone(),
    }));
    Ok(PageOps {
        number,
        width: page.width,
        height: page.height,
        ops,
    })
}

/// The layout's `pages` (zero-based, in the order given) as a display list. Every drawing
/// becomes one operation in the same order; each page's links follow its drawings, and its
/// signature fields follow its links. The document table is the whole layout's. Every face
/// of the layout is in the face table (so face ids mean the same in a partial list), each
/// with the glyphs the chosen pages use.
pub fn lower(
    layout: &Layout,
    fonts: &dyn FontPort,
    pages: &[usize],
) -> Result<DisplayList, PaintError> {
    let mut faces = parse_faces(layout, fonts)?;
    let pages = pages
        .iter()
        .map(|&n| {
            let p = layout.pages.get(n).ok_or(PaintError::NoPage(n))?;
            page(n, layout, p, &mut faces)
        })
        .collect::<Result<_, _>>()?;
    let faces = layout
        .faces
        .iter()
        .zip(faces)
        .map(|(key, built)| FaceEntry {
            key: key.clone(),
            units_per_em: built.face.units_per_em(),
            glyphs: built.glyphs,
        })
        .collect();
    let document = DocumentInfo {
        title: layout.title.clone(),
        language: layout.language.clone(),
        outline: layout.outline.clone(),
    };
    Ok(DisplayList {
        faces,
        document,
        pages,
    })
}

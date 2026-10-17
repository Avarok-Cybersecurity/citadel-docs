//! The layout IR lowered to display-list operations. The only translation is text: a run's
//! characters become glyph ids through the face's `cmap` (`.notdef`, 0, where it has none,
//! which is what the PDF draws), and each face entry collects the glyphs its runs use with
//! their advances in font units, so a painter places them by [`nominal_pen`]: no glyph drifts
//! more than half a hundredth of a point from where the PDF's `Tj` puts it, and a run's pen
//! ends exactly at the width the layout measured.

#[cfg(doc)]
use crate::op::nominal_pen;
use crate::op::{DisplayList, FaceEntry, GlyphMetric, Op, PageOps, Rgba, RunHead};
use crate::PaintError;
use cdoc_layout::{Draw, FontPort, Layout, Page, TextRun};

/// One face: parsed, with the glyph table being collected and a cache for ASCII.
struct FaceBuild<'a> {
    face: ttf_parser::Face<'a>,
    /// Glyph id → index in `glyphs`, `u32::MAX` for none yet.
    local: Vec<u32>,
    glyphs: Vec<GlyphMetric>,
    ascii: [Option<GlyphMetric>; 128],
}

impl FaceBuild<'_> {
    fn metric(&mut self, ch: char) -> GlyphMetric {
        let ascii = usize::try_from(u32::from(ch)).ok().filter(|i| *i < 128);
        if let Some(Some(m)) = ascii.map(|i| self.ascii[i]) {
            return m;
        }
        let id = self.face.glyph_index(ch).map_or(0, |g| g.0);
        let advance = self
            .face
            .glyph_hor_advance(ttf_parser::GlyphId(id))
            .unwrap_or(0);
        let m = GlyphMetric { id, advance };
        if let Some(i) = ascii {
            self.ascii[i] = Some(m);
        }
        m
    }

    fn register(&mut self, m: GlyphMetric) {
        let at = usize::from(m.id);
        if at >= self.local.len() {
            self.local.resize(at + 1, u32::MAX);
        }
        let slot = &mut self.local[at];
        if *slot == u32::MAX {
            *slot = u32::try_from(self.glyphs.len()).unwrap_or(u32::MAX);
            self.glyphs.push(m);
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
                local: vec![u32::MAX; usize::from(face.number_of_glyphs())],
                face,
                glyphs: Vec::new(),
                ascii: [None; 128],
            })
        })
        .collect()
}

fn text_run(run: &TextRun, faces: &mut [FaceBuild<'_>]) -> Result<Op, PaintError> {
    let face = faces
        .get_mut(usize::from(run.face.0))
        .ok_or(PaintError::NoFace(run.face.0))?;
    let glyphs = run
        .text
        .chars()
        .map(|ch| {
            let m = face.metric(ch);
            face.register(m);
            m.id
        })
        .collect();
    Ok(Op::TextRun {
        head: RunHead {
            face: run.face,
            size: run.size,
            colour: Rgba::opaque(run.colour),
            x: run.x,
            y: run.y,
        },
        glyphs,
    })
}

fn draw(d: &Draw, faces: &mut [FaceBuild<'_>]) -> Result<Op, PaintError> {
    let opaque = |c: &cdoc_model::Color| Rgba::opaque(*c);
    Ok(match d {
        Draw::Text(run) => text_run(run, faces)?,
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

fn page(number: usize, page: &Page, faces: &mut [FaceBuild<'_>]) -> Result<PageOps, PaintError> {
    let mut ops = Vec::with_capacity(page.draws.len() + page.links.len());
    for d in &page.draws {
        ops.push(draw(d, faces)?);
    }
    ops.extend(page.links.iter().map(|link| Op::Link {
        rect: link.rect,
        href: link.href.clone(),
    }));
    Ok(PageOps {
        number: u32::try_from(number).map_err(|_| PaintError::TooLarge)?,
        width: page.width,
        height: page.height,
        ops,
    })
}

/// The layout's `pages` (zero-based, in the order given) as a display list. Every drawing
/// becomes one operation in the same order; each page's links follow its drawings. Every face
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
            page(n, p, &mut faces)
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
    Ok(DisplayList { faces, pages })
}

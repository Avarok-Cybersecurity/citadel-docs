//! Shaping a 10 000-character Latin paragraph on Carlito: this shaper against rustybuzz, both
//! with a cached plan and reused buffers, both with the same features (Word's defaults; every
//! feature this shaper does not implement switched off in rustybuzz).

use cdoc_shape::{
    Direction, Face, Feature, Features, Language, Script, ShapeParams, ShapedRun, Shaper,
};
use criterion::{criterion_main, Criterion, Throughput};
use std::hint::black_box;

const CARLITO: &[u8] = include_bytes!("../../../fonts/Carlito-Regular.ttf");
const SEED: &str = "The office staff filed their affidavits: AVAST, To Tamworth! Wavy fjords, \
    fluffy waffles and a Yellow LTE flag. Quick zephyrs blow, vexing daft Jim; 1,234.56 per cent \
    of “VAT” is fine. ";

const NOT_OURS: [&[u8; 4]; 18] = [
    b"rvrn", b"ltra", b"ltrm", b"frac", b"numr", b"dnom", b"rand", b"trak", b"Harf", b"HARF",
    b"Buzz", b"BUZZ", b"abvm", b"blwm", b"rlig", b"curs", b"dist", b"rclt",
];

fn paragraph() -> String {
    SEED.chars().cycle().take(10_000).collect()
}

fn rb_features(features: Features) -> Vec<rustybuzz::Feature> {
    let tag = rustybuzz::ttf_parser::Tag::from_bytes;
    NOT_OURS
        .iter()
        .map(|t| rustybuzz::Feature::new(tag(t), 0, ..))
        .chain(Feature::ALL.iter().map(|f| {
            rustybuzz::Feature::new(
                tag(&f.tag().to_bytes()),
                u32::from(features.contains(*f)),
                ..,
            )
        }))
        .collect()
}

fn bench(c: &mut Criterion) {
    let text = paragraph();
    let features = Features::word_default();
    let params = ShapeParams {
        script: Script::Latin,
        language: Language::default_system(),
        direction: Direction::LeftToRight,
        features,
    };
    let mut group = c.benchmark_group("shape 10k Latin chars, Carlito");
    group.throughput(Throughput::Elements(10_000));

    let face = Face::parse(CARLITO, 0).expect("face");
    let mut shaper = Shaper::new(&face);
    let mut run = ShapedRun::empty();
    group.bench_function("cdoc-shape", |b| {
        b.iter(|| {
            shaper
                .shape_into(black_box(&text), &params, &mut run)
                .expect("shapes");
            black_box(run.glyphs.len())
        });
    });

    let rb = rustybuzz::Face::from_slice(CARLITO, 0).expect("face");
    let rb_feats = rb_features(features);
    let plan = rustybuzz::ShapePlan::new(
        &rb,
        rustybuzz::Direction::LeftToRight,
        Some(rustybuzz::script::LATIN),
        None,
        &rb_feats,
    );
    let mut spare = Some(rustybuzz::UnicodeBuffer::new());
    group.bench_function("rustybuzz", |b| {
        b.iter(|| {
            let mut buffer = spare.take().expect("buffer");
            buffer.push_str(black_box(&text));
            buffer.set_direction(rustybuzz::Direction::LeftToRight);
            buffer.set_script(rustybuzz::script::LATIN);
            let output = rustybuzz::shape_with_plan(&rb, &plan, buffer);
            let glyph_count = output.len();
            spare = Some(output.clear());
            black_box(glyph_count)
        });
    });
    group.finish();
}

/// Runs the group with Criterion's command-line configuration.
fn benches() {
    let mut c = Criterion::default().configure_from_args();
    bench(&mut c);
}

criterion_main!(benches);

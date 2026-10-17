//! The CRDT: the model into a `cdoc-crdt` document, its snapshot out and back in, one keystroke
//! into a document of each size, and plain typing of 10 000 characters (the workload yrs is
//! measured on too, in `competitors`).

use super::{paged, BenchError, Ctx, Family, Fixture, Results, CRDT_TYPE, CRDT_TYPE_CHARS};
use crate::schema::Measurement;
use cdoc_crdt::{from_model, ClientId, Doc};
use std::hint::black_box;

fn crdt(e: &dyn std::fmt::Display) -> BenchError {
    BenchError::Crdt(e.to_string())
}

/// Types `chars` single characters, each at the end, into a fresh document.
pub fn type_chars(chars: usize) -> Result<Doc, BenchError> {
    let mut d = Doc::new(ClientId(1));
    for i in 0..chars {
        d.insert_text(i, "a").map_err(|e| crdt(&e))?;
    }
    Ok(d)
}

pub(super) fn run(ctx: &Ctx<'_>, fx: &Fixture, out: &mut Results) -> Result<(), BenchError> {
    let key = paged(Family::CrdtFromModel, fx.pages);
    let t = ctx.time(&key, || {
        black_box(from_model(black_box(&fx.doc), ClientId(1)).map_err(|e| crdt(&e))?);
        Ok(())
    })?;
    out.insert(key, Measurement::Time(t));

    let mut doc = from_model(&fx.doc, ClientId(1)).map_err(|e| crdt(&e))?;
    let key = paged(Family::CrdtSnapshotEncode, fx.pages);
    let t = ctx.time(&key, || {
        black_box(doc.encode_snapshot());
        Ok(())
    })?;
    out.insert(key, Measurement::Time(t));

    let snapshot = doc.encode_snapshot();
    let key = paged(Family::CrdtSnapshotDecode, fx.pages);
    let t = ctx.time(&key, || {
        let back = Doc::decode_snapshot(black_box(&snapshot), ClientId(2)).map_err(|e| crdt(&e))?;
        black_box(back);
        Ok(())
    })?;
    out.insert(key, Measurement::Time(t));

    let key = paged(Family::CrdtKeystroke, fx.pages);
    let t = ctx.time(&key, || {
        let middle = doc.len() / 2;
        doc.insert_text(middle, "x").map_err(|e| crdt(&e))?;
        Ok(())
    })?;
    out.insert(key, Measurement::Time(t));
    Ok(())
}

pub(super) fn run_typing(ctx: &Ctx<'_>, out: &mut Results) -> Result<(), BenchError> {
    let t = ctx.time(CRDT_TYPE, || {
        black_box(type_chars(CRDT_TYPE_CHARS)?);
        Ok(())
    })?;
    out.insert(CRDT_TYPE.to_owned(), Measurement::Time(t));
    Ok(())
}

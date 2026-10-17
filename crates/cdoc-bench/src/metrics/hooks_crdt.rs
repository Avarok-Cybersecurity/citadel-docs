//! `cdoc-crdt` (feature `crdt`; compiled only once the crate is on `main`, see [`super`]): the
//! model into a CRDT document, a snapshot out and back in, and one keystroke into a document
//! of each size. Its competitor, yrs, joins `competitors` when this lands.

use super::super::{paged_name, BenchError, Ctx, Fixture, Results};
use crate::schema::Measurement;
use cdoc_crdt::{from_model, ClientId, Doc};
use std::hint::black_box;

fn hook(message: &dyn std::fmt::Display) -> BenchError {
    BenchError::Hook {
        family: "crdt",
        message: message.to_string(),
    }
}

pub(super) fn run(ctx: &Ctx<'_>, fx: &Fixture, out: &mut Results) -> Result<(), BenchError> {
    let key = paged_name("crdt_from_model", fx.pages);
    let t = ctx.time(&key, || {
        black_box(from_model(black_box(&fx.doc), ClientId(1)).map_err(|e| hook(&e))?);
        Ok(())
    })?;
    out.insert(key, Measurement::Time(t));

    let mut doc = from_model(&fx.doc, ClientId(1)).map_err(|e| hook(&e))?;
    let key = paged_name("crdt_snapshot_encode", fx.pages);
    let t = ctx.time(&key, || {
        black_box(doc.encode_snapshot());
        Ok(())
    })?;
    out.insert(key, Measurement::Time(t));

    let snapshot = doc.encode_snapshot();
    let key = paged_name("crdt_snapshot_decode", fx.pages);
    let t = ctx.time(&key, || {
        black_box(Doc::decode_snapshot(black_box(&snapshot), ClientId(2)).map_err(|e| hook(&e))?);
        Ok(())
    })?;
    out.insert(key, Measurement::Time(t));

    let key = paged_name("crdt_keystroke", fx.pages);
    let t = ctx.time(&key, || {
        let middle = doc.len() / 2;
        doc.insert_text(middle, "x").map_err(|e| hook(&e))?;
        Ok(())
    })?;
    out.insert(key, Measurement::Time(t));
    Ok(())
}

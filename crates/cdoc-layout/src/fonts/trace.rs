//! What an incremental layout needs from the faces: which requests a stretch of work made (so
//! the order a fresh layout would meet faces in can be replayed), and a rebase that renumbers
//! the faces into that order and drops the ones no longer used.

use super::Fonts;
use crate::ir::{FaceId, FaceKey};

impl Fonts<'_> {
    /// The requests made since the last call, each once, in the order first made.
    pub fn take_requests(&mut self) -> Vec<usize> {
        let mut out: Vec<usize> = Vec::with_capacity(self.log.len().min(8));
        for index in self.log.drain(..) {
            if !out.contains(&index) {
                out.push(index);
            }
        }
        out
    }

    /// The face that served request `index`, unless a rebase dropped it.
    #[must_use]
    pub fn served(&self, index: usize) -> Option<FaceId> {
        self.requests.get(index)?.as_ref().map(|(_, id)| *id)
    }

    /// The request and the different face that served it, when it was a substitution.
    #[must_use]
    pub fn substitution(&self, index: usize) -> Option<(FaceKey, FaceKey)> {
        let (requested, id) = self.requests.get(index)?.as_ref()?;
        let served = &self.keys[usize::from(id.0)];
        let same = served.family.eq_ignore_ascii_case(&requested.family)
            && served.bold == requested.bold
            && served.italic == requested.italic;
        (!same).then(|| (requested.clone(), served.clone()))
    }

    /// Renumbers the faces so that `order[n]` (an id as it is now) becomes `FaceId(n)`, and
    /// forgets every face not in `order` together with the requests it served. Returns the new
    /// id of every old id, `None` for a dropped face.
    pub fn rebase(&mut self, order: &[FaceId]) -> Vec<Option<FaceId>> {
        let mut map = vec![None; self.keys.len()];
        let mut parsed: Vec<_> = std::mem::take(&mut self.parsed)
            .into_iter()
            .map(Some)
            .collect();
        let mut keys: Vec<_> = std::mem::take(&mut self.keys)
            .into_iter()
            .map(Some)
            .collect();
        for (new, old) in order.iter().enumerate() {
            let old = usize::from(old.0);
            let (Some(p), Some(k)) = (parsed[old].take(), keys[old].take()) else {
                continue;
            };
            self.parsed.push(p);
            self.keys.push(k);
            map[old] = u16::try_from(new).ok().map(FaceId);
        }
        for slot in &mut self.requests {
            let new = slot.as_ref().and_then(|(_, id)| map[usize::from(id.0)]);
            match (new, slot.as_mut()) {
                (Some(new), Some((_, id))) => *id = new,
                _ => *slot = None,
            }
        }
        let requests = &self.requests;
        self.by_request
            .retain(|_, index| requests[*index].is_some());
        self.log.clear();
        map
    }
}

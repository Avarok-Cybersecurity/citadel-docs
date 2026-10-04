//! The appended cross-reference section and trailer: a table when the file ended with a table,
//! a stream when it ended with a stream (a reader that follows `/Prev` from one kind into the
//! other is not one every viewer is).

use super::tail::Tail;
use std::fmt::Write as _;

/// Runs of consecutive object numbers, as `(first, entries)`.
fn runs(entries: &[(i32, usize)]) -> Vec<(i32, Vec<(i32, usize)>)> {
    let mut out: Vec<(i32, Vec<(i32, usize)>)> = Vec::new();
    for &(num, offset) in entries {
        match out.last_mut() {
            Some((first, run)) if *first + i32::try_from(run.len()).unwrap_or(0) == num => {
                run.push((num, offset));
            }
            _ => out.push((num, vec![(num, offset)])),
        }
    }
    out
}

fn trailer_entries(tail: &Tail, size: i32) -> String {
    let mut t = format!(
        "/Size {size} /Root {} {} R /Prev {}",
        tail.root.obj_number, tail.root.gen_number, tail.startxref
    );
    if let Some(info) = tail.info {
        let _ = write!(t, " /Info {} {} R", info.obj_number, info.gen_number);
    }
    if let Some(id) = &tail.id {
        t.push_str(" /ID ");
        t.push_str(&String::from_utf8_lossy(id));
    }
    t
}

/// Appends the section for `entries` (object number, offset; every one generation 0) and the
/// end-of-file marker. `next` is the first unused object number.
pub fn finish(out: &mut Vec<u8>, entries: &mut Vec<(i32, usize)>, tail: &Tail, next: i32) {
    if tail.stream {
        stream(out, entries, tail, next);
        return;
    }
    entries.sort_unstable();
    let at = out.len();
    let mut table = String::from("xref\n");
    for (first, run) in runs(entries) {
        let _ = writeln!(table, "{first} {}", run.len());
        for (_, offset) in run {
            let _ = write!(table, "{offset:010} 00000 n\r\n");
        }
    }
    let _ = write!(
        table,
        "trailer\n<<{}>>\nstartxref\n{at}\n%%EOF\n",
        trailer_entries(tail, next)
    );
    out.extend_from_slice(table.as_bytes());
}

fn stream(out: &mut Vec<u8>, entries: &mut Vec<(i32, usize)>, tail: &Tail, next: i32) {
    let at = out.len();
    entries.push((next, at));
    entries.sort_unstable();
    let mut rows = Vec::with_capacity(entries.len() * 7);
    let mut index = String::new();
    for (first, run) in runs(entries) {
        let _ = write!(index, "{first} {} ", run.len());
        for (_, offset) in run {
            rows.push(1);
            rows.extend_from_slice(&u32::try_from(offset).unwrap_or(u32::MAX).to_be_bytes());
            rows.extend_from_slice(&[0, 0]);
        }
    }
    let head = format!(
        "{next} 0 obj\n<</Type /XRef {} /W [1 4 2] /Index [{}] /Length {}>>\nstream\n",
        trailer_entries(tail, next + 1),
        index.trim_end(),
        rows.len()
    );
    out.extend_from_slice(head.as_bytes());
    out.extend_from_slice(&rows);
    out.extend_from_slice(format!("\nendstream\nendobj\nstartxref\n{at}\n%%EOF\n").as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consecutive_numbers_share_a_subsection() {
        let r = runs(&[(3, 10), (4, 20), (7, 30)]);
        assert_eq!(r, vec![(3, vec![(3, 10), (4, 20)]), (7, vec![(7, 30)])]);
    }
}

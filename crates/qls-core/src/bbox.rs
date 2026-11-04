//! FLUX.3's layout contract. BFL's FLUX.3 Image models take their layout as a JSON table
//! *inside the prompt string*: `"<caption> [rows]"`, each row placing one element on a
//! 0-1000 canvas (`[y_min, x_min, y_max, x_max]`, top-left origin). No engine magic — the
//! table is text the encoder reads. This is a byte-compatible port of BFL's reference
//! `bbox.py`: the table the UI appends to the prompt, the locked-annotation form the
//! prompt upsampler receives, and the round-trip that re-imposes the user's boxes on the
//! upsampler's answer before the DiT sees it.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The canvas the boxes live on: 0-1000, `[y_min, x_min, y_max, x_max]`, top-left origin.
pub const SCALE: i64 = 1000;

fn id_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z][A-Za-z0-9_]*$").unwrap())
}

fn origin_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(?:ref_image_\d+|target)$").unwrap())
}

/// The upsampler's locked-input form, as a full-match on the text before the table.
fn locked_form_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?s)^ORIGINAL PROMPT:\n(.*?)\n\nLOCKED BOUNDING-BOX ANNOTATIONS .*$").unwrap()
    })
}

fn is_id(s: &str) -> bool {
    id_re().is_match(s)
}

fn is_origin(s: &str) -> bool {
    origin_re().is_match(s)
}

/// The box, if it is valid on the 0-1000 canvas.
///
/// Mirrors BFL `bbox.valid_box`: exactly 4 numbers, `y2 > y1` and `x2 > x1`, all in
/// `0..=1000`. Rejects 0-1 coordinates (every value ≤ 1) — the UI's normalized form — as
/// well as booleans, non-integral floats and the wrong length.
pub fn box_from_value(v: &Value) -> Option<[i64; 4]> {
    let arr = v.as_array()?;
    if arr.len() != 4 {
        return None;
    }
    let mut out = [0i64; 4];
    for (i, e) in arr.iter().enumerate() {
        // `as_i64` rejects booleans and fractional numbers; whole-valued floats (5.0)
        // pass, as BFL's `isinstance(v, (int, float))` accepts them.
        out[i] = e
            .as_i64()
            .or_else(|| {
                let f = e.as_f64()?;
                (f.is_finite() && f.fract() == 0.0 && f >= i64::MIN as f64 && f <= i64::MAX as f64)
                    .then_some(f as i64)
            })?;
    }
    let [y1, x1, y2, x2] = out;
    if y1 < 0
        || x1 < 0
        || y2 <= y1
        || x2 <= x1
        || y2 > SCALE
        || x2 > SCALE
        || *out.iter().max()? <= 1
    {
        return None;
    }
    Some(out)
}

/// `(caption, rows)`: the last `[` from which the rest parses as a JSON list.
///
/// Mirrors BFL `bbox.split` — a scan, not a regex: descriptions may contain brackets and
/// escaped quotes, and only a suffix that is exactly a JSON list counts.
pub fn split(text: &str) -> (String, Option<Vec<Value>>) {
    for i in (0..text.len()).rev() {
        if text.as_bytes()[i] == b'[' {
            if let Ok(v) = serde_json::from_str::<Value>(&text[i..]) {
                if let Some(list) = v.as_array() {
                    return (text[..i].trim_end().to_string(), Some(list.clone()));
                }
            }
        }
    }
    (text.trim().to_string(), None)
}

/// `(caption, rows)` when the prompt already carries boxes on the 0-1000 canvas, else
/// `None`. Two forms are recognized: the DiT form `"<caption> [rows]"` and the
/// upsampler's own locked input (in which case the caption is the embedded original
/// prompt). A trailing list that is not all valid box rows (e.g. a caption ending in
/// `"[SALE]"`) is not a box table.
pub fn detect(prompt: &str) -> Option<(String, Vec<Value>)> {
    let (text, rows) = split(prompt);
    let rows = rows?;
    if rows.is_empty()
        || !rows.iter().all(|r| r.as_object().is_some_and(|o| {
            o.get("id").and_then(Value::as_str).is_some_and(is_id)
                && [o.get("bbox"), o.get("src_bbox"), o.get("tgt_bbox")]
                    .iter()
                    .any(|v| v.is_some_and(|v| box_from_value(v).is_some()))
        }))
    {
        return None;
    }
    let caption = locked_form_re()
        .captures(&text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or(text);
    Some((caption, rows))
}

/// An upsampler answer, not a request: its box table is cited in full as `<id>`s in the
/// caption. BFL skips re-upsampling such a prompt — the DiT gets it as written.
pub fn is_finished(prompt: &str) -> bool {
    let Some((caption, rows)) = detect(prompt) else {
        return false;
    };
    rows.iter()
        .filter_map(|r| r.get("id").and_then(Value::as_str))
        .all(|id| caption.contains(&format!("<{id}>")))
}

/// A table row in BFL's wire form. `T2i` is a text-to-image row (an element of the image
/// to be generated); `Edit` is an image-edit row (where the element comes from and where
/// it goes). Serialized in BFL's key order; `src_bbox`/`tgt_bbox` are `null` on the side
/// the row has no box on — the nulls are part of the contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BoxRow {
    T2i {
        id: String,
        bbox: [i64; 4],
        desc: String,
    },
    Edit {
        id: String,
        from: String,
        src_bbox: Option<[i64; 4]>,
        tgt_bbox: Option<[i64; 4]>,
        desc: String,
    },
}

impl BoxRow {
    pub fn id(&self) -> &str {
        match self {
            BoxRow::T2i { id, .. } | BoxRow::Edit { id, .. } => id,
        }
    }

    /// The box used for identity matching: the t2i box, else the target box, else the
    /// source box (BFL `bbox.reconcile` uses the same order).
    pub fn identity_box(&self) -> Option<&[i64; 4]> {
        match self {
            BoxRow::T2i { bbox, .. } => Some(bbox),
            BoxRow::Edit {
                src_bbox,
                tgt_bbox,
                ..
            } => tgt_bbox.as_ref().or(src_bbox.as_ref()),
        }
    }
}

fn set_desc(row: &mut BoxRow, desc: &str) {
    match row {
        BoxRow::T2i { desc: d, .. } | BoxRow::Edit { desc: d, .. } => *d = desc.into(),
    }
}

/// A detected row kept as the user wrote it. BFL's `plan` does not re-validate a
/// detected row's `from` (only its shape is normalized), so the locked rows survive
/// verbatim even when the `from` value is unusual.
pub fn locked_row(v: &Value) -> Option<BoxRow> {
    let o = v.as_object()?;
    let id = o.get("id")?.as_str()?;
    if !is_id(id) {
        return None;
    }
    let desc = o
        .get("desc")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if let Some(bbox) = o.get("bbox").and_then(|b| box_from_value(b)) {
        return Some(BoxRow::T2i {
            id: id.into(),
            bbox,
            desc,
        });
    }
    let src = o.get("src_bbox").and_then(|b| box_from_value(b));
    let tgt = o.get("tgt_bbox").and_then(|b| box_from_value(b));
    (src.is_some() || tgt.is_some()).then_some(BoxRow::Edit {
        id: id.into(),
        from: o
            .get("from")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        src_bbox: src,
        tgt_bbox: tgt,
        desc,
    })
}

/// A row the upsampler added, if it may be kept: its id is cited in the caption as
/// `<id>`, its description is non-empty, and it is well formed (BFL `bbox.valid_added`).
pub fn valid_added(v: &Value, caption: &str) -> Option<BoxRow> {
    let o = v.as_object()?;
    let id = o.get("id")?.as_str()?;
    if !is_id(id) || !caption.contains(&format!("<{id}>")) {
        return None;
    }
    let desc = o.get("desc")?.as_str()?;
    let desc = desc.trim();
    if desc.is_empty() {
        return None;
    }
    if let Some(bbox) = o.get("bbox").and_then(|b| box_from_value(b)) {
        return Some(BoxRow::T2i {
            id: id.into(),
            bbox,
            desc: desc.into(),
        });
    }
    let from = o.get("from")?.as_str()?;
    // A box present in the JSON but malformed is a rejection, like an absent one on a
    // row that has no box at all.
    let raw_src = o.get("src_bbox").filter(|v| !v.is_null());
    let raw_tgt = o.get("tgt_bbox").filter(|v| !v.is_null());
    let src = raw_src.and_then(|b| box_from_value(b));
    let tgt = raw_tgt.and_then(|b| box_from_value(b));
    let ok = is_origin(from)
        && (raw_src.is_none() || src.is_some())
        && (raw_tgt.is_none() || tgt.is_some())
        && (src.is_some() || tgt.is_some())
        && (from == "target") == (src.is_none());
    ok.then_some(BoxRow::Edit {
        id: id.into(),
        from: from.into(),
        src_bbox: src,
        tgt_bbox: tgt,
        desc: desc.into(),
    })
}

/// Intersection over union of two `[y1, x1, y2, x2]` boxes (BFL `bbox.iou`).
pub fn iou(a: &[i64; 4], b: &[i64; 4]) -> f64 {
    let inter = (a[2].min(b[2]) - a[0].max(b[0])).max(0) as f64
        * (a[3].min(b[3]) - a[1].max(b[1])).max(0) as f64;
    let union = (a[2] - a[0]) as f64 * (a[3] - a[1]) as f64
        + (b[2] - b[0]) as f64 * (b[3] - b[1]) as f64
        - inter;
    if union > 0.0 {
        inter / union
    } else {
        0.0
    }
}

/// The locked-annotation user text the upsampler receives when the request carries the
/// user's boxes: the original prompt, then the rows it must preserve exactly.
/// Byte-compatible with BFL `bbox.upsampling_input`.
pub fn upsampling_input(prompt: &str, rows: &[BoxRow]) -> String {
    format!(
        "ORIGINAL PROMPT:\n{}\n\nLOCKED BOUNDING-BOX ANNOTATIONS ([y_min, x_min, y_max, x_max] on a 0-1000 canvas, with (0, 0) at the top-left). This is an authoritative JSON list of dictionaries; preserve every supplied identity, provenance value, null, and coordinate exactly:\n{}",
        prompt.trim(),
        serde_json::to_string(rows).unwrap_or_else(|_| "[]".into())
    )
}

/// The upsampler's answer with the user's boxes re-imposed (BFL `bbox.reconcile`):
///
/// 1. A row that re-plans a locked box under its own id (IoU ≥ 0.9, e.g. `balloon_1`
///    for `obj_1`) takes the locked id, in the rows and in the caption's `<id>`
///    citations.
/// 2. Locked rows are kept verbatim, adopting the upsampler's description where it wrote
///    one; the upsampler's own rows follow, if valid.
///
/// The return is the DiT prompt: the caption, one space, the table.
pub fn reconcile(text: &str, locked: &[BoxRow]) -> String {
    let (mut caption, rows) = split(text);
    let mut rows = rows.unwrap_or_default();
    let locked_ids: HashSet<&str> = locked.iter().map(|r| r.id()).collect();
    let row_ids: HashSet<String> = rows
        .iter()
        .filter_map(|r| r.get("id").and_then(Value::as_str))
        .map(|s| s.to_string())
        .collect();

    // 1. Rows that re-plan a locked box take the locked id.
    let mut renamed: Vec<(String, String)> = Vec::new();
    for lock in locked {
        let Some(lock_box) = lock.identity_box() else {
            continue;
        };
        if row_ids.contains(lock.id()) {
            continue;
        }
        for row in &rows {
            let Some(row_id) = row.get("id").and_then(Value::as_str) else {
                continue;
            };
            if renamed.iter().any(|(old, _)| old == row_id) || locked_ids.contains(row_id) {
                continue;
            }
            let row_box = row
                .get("bbox")
                .or_else(|| row.get("tgt_bbox"))
                .or_else(|| row.get("src_bbox"))
                .and_then(|v| box_from_value(v));
            if row_box.is_some_and(|b| iou(&b, lock_box) >= 0.9) {
                renamed.push((row_id.to_string(), lock.id().to_string()));
                break;
            }
        }
    }
    let renames: HashSet<(String, String)> = renamed.iter().cloned().collect();
    for row in &mut rows {
        if let Some(o) = row.as_object_mut() {
            if let Some(id) = o.get("id").and_then(Value::as_str) {
                if let Some((_, new)) = renames.iter().find(|(old, _)| old == id) {
                    o.insert("id".into(), Value::String(new.clone()));
                }
            }
        }
    }
    for (old, new) in &renamed {
        caption = caption.replace(&format!("<{old}>"), &format!("<{new}>"));
    }

    let by_id: HashMap<&str, &Value> = rows
        .iter()
        .filter_map(|r| r.get("id").and_then(Value::as_str).map(|id| (id, r)))
        .collect();

    // 2. Locked rows verbatim (adopting the upsampler's description), then its own.
    let mut out: Vec<BoxRow> = Vec::new();
    for lock in locked {
        let mut row = lock.clone();
        if let Some(desc) = by_id
            .get(lock.id())
            .and_then(|r| r.get("desc"))
            .and_then(Value::as_str)
        {
            if !desc.trim().is_empty() {
                set_desc(&mut row, desc.trim());
            }
        }
        out.push(row);
    }
    for row in &rows {
        if let Some(added) = valid_added(row, &caption) {
            if out.iter().any(|r| r.id() == added.id()) {
                continue;
            }
            // A row that re-covers an element a locked row already owns only
            // duplicates the instruction; the locked row stays authoritative.
            let dup = added.identity_box().is_some_and(|ab| {
                locked.iter().any(|l| l.identity_box().is_some_and(|lb| iou(&ab, &lb) >= 0.5))
            });
            if !dup {
                out.push(added);
            }
        }
    }
    format!(
        "{caption} {}",
        serde_json::to_string(&out).unwrap_or_else(|_| "[]".into())
    )
    .trim()
    .to_string()
}

/// The DiT follows the caption's sentences, not the table: a locked removal (a target
/// box of null) phrased as a change ("change X to a dark wall") or a move leaves the
/// object standing. The clause that talks about the id is rewritten into an
/// unambiguous removal; already-removed ids are left alone, and a caption without
/// any locked removal passes through untouched.
/// The DiT follows the caption's sentences, not the table: if the upsampler
/// reworded a locked removal as a change ("change the background X to a dark
/// wall"), the object stays. The clause that talks about the id is rewritten
/// into an unambiguous removal; a sentence is appended only when the id is
/// missing entirely or the clause is shared with another id.
pub fn fix_remove_actions(caption: &str, locked: &[BoxRow]) -> String {
    let mut out = caption.to_string();
    for r in locked.iter().filter(|r| matches!(r, BoxRow::Edit { tgt_bbox: None, .. })) {
        let id = r.id();
        let tag = format!("<{id}>");
        let desc = match r {
            BoxRow::Edit { desc, .. } => desc.trim().to_string(),
            _ => unreachable!(),
        };
        let mut noun = if desc.is_empty() { "the object".to_string() } else { desc };
        // A bare noun ("mug") reads wrong in "Remove mug <id>": give it a definite article.
        if !noun.starts_with("a ") && !noun.starts_with("an ") && !noun.starts_with("the ") {
            noun = format!("the {noun}");
        }

        // A sentence that already says removed / gone / no longer stands.
        let ok = out
            .split(|c: char| matches!(c, '.' | '!' | '?'))
            .any(|s| s.contains(&tag) && {
                let l = s.to_lowercase();
                l.contains("remov") || l.contains("no longer") || l.contains(" gone") || l.contains("without")
            });
        if ok {
            continue;
        }

        // The clause around the id: from after the previous delimiter to the
        // first delimiter after it.
        let Some(start) = out.find(&tag) else {
            out = format!("{out} Remove {noun} {tag} from the scene.");
            continue;
        };
        let clause_end = out
            .get(start + tag.len()..)
            .and_then(|rest| rest.find(|c: char| matches!(c, ',' | '.' | '!' | '?')))
            .map(|i| start + tag.len() + i)
            .unwrap_or(out.len());
        let clause = out[start..clause_end].to_string();
        let delim = if clause_end < out.len() {
            out.as_bytes()[clause_end] as char
        } else {
            '.'
        };
        let c_begin = out[..start]
            .rfind(|c: char| matches!(c, ',' | '.' | '!' | '?'))
            .map(|i| i + 1)
            .unwrap_or(0);

        // Another <id> in the same clause: replacing it would eat that
        // instruction, so append instead of rewrite.
        let shared = tag_tokens(&clause).into_iter().any(|t| t != id);
        if shared {
            out = format!("{out} Remove {noun} {tag} from the scene.");
            continue;
        }

        let lc = out[c_begin..clause_end].trim_start().to_lowercase();
        let lead = if lc.starts_with("and ") {
            "and "
        } else if lc.starts_with("while ") {
            "while "
        } else if lc.starts_with("plus ") {
            "plus "
        } else if lc.starts_with("then ") {
            "then "
        } else {
            ""
        };
        let (verb, sep) = if lead.is_empty() {
            if c_begin == 0 {
                ("Remove", "")
            } else {
                ("remove", " ")
            }
        } else {
            (lead, "")
        };
        let phrase = format!("{sep}{verb} {noun} {tag} from the scene");
        let tail = if delim == ',' { ',' } else { '.' };
        let skip = if clause_end < out.len() { 1 } else { 0 };
        out = format!("{}{}{}{}", &out[..c_begin], phrase, tail, &out[clause_end + skip..]);
    }
    out
}

/// The <...> tokens in a piece of text, in order.
fn tag_tokens(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(a) = rest.find('<') {
        let after = &rest[a + 1..];
        match after.find('>') {
            Some(b) => {
                out.push(after[..b].to_string());
                rest = &after[b + 1..];
            }
            None => break,
        }
    }
    out
}

#[cfg(test)]
mod tests {
    fn edit_row_fixture(id: &str, desc: &str, src: Option<[i64; 4]>, tgt: Option<[i64; 4]>) -> BoxRow {
        BoxRow::Edit { id: id.into(), from: "ref_image_0".into(), src_bbox: src, tgt_bbox: tgt, desc: desc.into() }
    }
    use super::*;
    use serde_json::json;

    fn t2i(id: &str, box_: [i64; 4], desc: &str) -> BoxRow {
        BoxRow::T2i {
            id: id.into(),
            bbox: box_,
            desc: desc.into(),
        }
    }

    #[test]
    fn split_takes_the_last_parseable_list() {
        // The last '[' wins, even when earlier brackets and JSON-looking text exist.
        let (c, r) = split("a [SALE] sign, then [1, 2] mid-text, and at the end [1, 2, 3]");
        assert_eq!(c, "a [SALE] sign, then [1, 2] mid-text, and at the end");
        assert_eq!(
            r.unwrap().len(),
            3,
            "the final bracket is the real table"
        );
        // Escaped quotes and brackets inside a description do not confuse the scan.
        let (c, r) = split(
            r#"says "hello [world]" here [{"id": "a_1", "bbox": [1, 2, 3, 4], "desc": "a [thing]"}]"#,
        );
        assert_eq!(c, r#"says "hello [world]" here"#);
        assert_eq!(r.unwrap().len(), 1);
        // A trailing object, not a list, is not a table: the text stays whole.
        let (c, r) = split("end with {\"id\": \"a_1\"}");
        assert_eq!(c, "end with {\"id\": \"a_1\"}");
        assert!(r.is_none());
        // Plain text.
        let (c, r) = split("  nothing here  ");
        assert_eq!((c.as_str(), r.is_none()), ("nothing here", true));
    }

    #[test]
    fn valid_box_accepts_and_rejects() {
        assert_eq!(
            box_from_value(&json!([0, 0, 1000, 1000])),
            Some([0, 0, 1000, 1000]),
            "full canvas is valid"
        );
        assert_eq!(
            box_from_value(&json!([10.0, 20, 30.5, 40])),
            None,
            "a fractional coordinate is not a canvas grid"
        );
        assert_eq!(
            box_from_value(&json!([10, 20, 30, 40.0])),
            Some([10, 20, 30, 40]),
            "a whole-valued float passes, like BFL's isinstance check"
        );
        assert_eq!(box_from_value(&json!([0, 0, 1, 1])), None, "0-1 coordinates");
        assert_eq!(box_from_value(&json!([5, 5, 5, 5])), None, "zero height");
        assert_eq!(box_from_value(&json!([5, 5, 4, 5])), None, "inverted");
        assert_eq!(box_from_value(&json!([-1, 0, 5, 5])), None, "negative");
        assert_eq!(box_from_value(&json!([0, 0, 1001, 5])), None, "beyond the canvas");
        assert_eq!(box_from_value(&json!([0, 0, 5, 5, 5])), None, "five numbers");
        assert_eq!(box_from_value(&json!([true, 0, 5, 5])), None, "a boolean");
        assert_eq!(box_from_value(&json!(["1", 0, 5, 5])), None, "a string");
        assert_eq!(box_from_value(&json!(null)), None);
    }

    #[test]
    fn detect_finds_both_forms_and_rejects_the_rest() {
        let table = json!([
            {"id": "lamp_1", "bbox": [100, 200, 500, 600], "desc": "a brass lamp"},
            {"id": "cat_2", "bbox": [0, 0, 300, 400], "desc": "a cat"},
        ]);
        let prompt = format!(
            "A lamp <lamp_1> and a cat <cat_2>. {}",
            serde_json::to_string(&table).unwrap()
        );
        let (c, r) = detect(&prompt).unwrap();
        assert_eq!(c, "A lamp <lamp_1> and a cat <cat_2>.");
        assert_eq!(r.len(), 2);

        // The locked input form: the caption is the embedded original prompt.
        let locked = upsampling_input("make the lamp red", &[t2i("lamp_1", [1, 2, 3, 4], "d")]);
        let prompt = format!("{locked} {}", serde_json::to_string(&[t2i("lamp_1", [1, 2, 3, 4], "d")]).unwrap());
        let (c, r) = detect(&prompt).unwrap();
        assert_eq!(c, "make the lamp red");
        assert_eq!(r.len(), 1);

        // A trailing list that is not all box rows is not a table.
        assert!(detect("a caption ending in [SALE]").is_none());
        assert!(detect("x [1, 2]").is_none(), "a bare number list has no rows");
        assert!(detect("x []").is_none(), "an empty list is not a table");
        assert!(
            detect("x [{\"id\": \"1bad\", \"bbox\": [1, 2, 3, 4], \"desc\": \"d\"}]").is_none(),
            "an id must start with a letter"
        );
        assert!(
            detect("x [{\"id\": \"a_1\", \"bbox\": [0.1, 0.2, 0.3, 0.4], \"desc\": \"d\"}]").is_none(),
            "0-1 boxes are not canvas boxes"
        );
        assert!(
            detect("x [{\"id\": \"a_1\", \"desc\": \"no box at all\"}]").is_none()
        );
    }

    #[test]
    fn finished_means_every_table_id_is_cited() {
        let t = json!([{"id": "a_1", "bbox": [1, 2, 3, 4], "desc": "d"}]);
        let s = serde_json::to_string(&t).unwrap();
        assert!(is_finished(&format!("A thing <a_1>. {s}")));
        assert!(!is_finished(&format!("A thing. {s}")), "uncited id");
        assert!(!is_finished("no table at all"));
    }

    #[test]
    fn row_parsing_keeps_the_wire_form() {
        let v = json!({"id": "c_1", "from": "ref_image_0", "src_bbox": [1, 2, 3, 4], "tgt_bbox": null, "desc": "the cup"});
        assert_eq!(
            locked_row(&v),
            Some(BoxRow::Edit {
                id: "c_1".into(),
                from: "ref_image_0".into(),
                src_bbox: Some([1, 2, 3, 4]),
                tgt_bbox: None,
                desc: "the cup".into(),
            })
        );
        // The strict parser (rows the model adds) rejects an impossible `from`.
        let v = json!({"id": "c_1", "from": "nowhere", "src_bbox": [1, 2, 3, 4], "tgt_bbox": [1, 2, 3, 4], "desc": "d"});
        assert!(valid_added(&v, "a cup <c_1>").is_none());
        // `target` rows have no source box, `ref_image_N` rows do.
        let v = json!({"id": "c_1", "from": "target", "src_bbox": [1, 2, 3, 4], "tgt_bbox": [1, 2, 3, 4], "desc": "d"});
        assert!(valid_added(&v, "a cup <c_1>").is_none());
        let v = json!({"id": "c_1", "from": "ref_image_1", "src_bbox": null, "tgt_bbox": [1, 2, 3, 4], "desc": "d"});
        assert!(valid_added(&v, "a cup <c_1>").is_none());
        let v = json!({"id": "c_1", "from": "ref_image_1", "src_bbox": [1, 2, 3, 4], "tgt_bbox": [5, 6, 7, 8], "desc": "d"});
        assert!(valid_added(&v, "a cup <c_1>").is_some());
        // An uncited id and an empty desc are rejected.
        let v = json!({"id": "c_1", "bbox": [1, 2, 3, 4], "desc": "d"});
        assert!(valid_added(&v, "no citation").is_none());
        let v = json!({"id": "c_1", "bbox": [1, 2, 3, 4], "desc": "  "});
        assert!(valid_added(&v, "a cup <c_1>").is_none());
    }

    #[test]
    fn serialization_is_in_bfl_key_order_with_explicit_nulls() {
        let rows = vec![
            t2i("person_1", [10, 20, 300, 400], "a person"),
            BoxRow::Edit {
                id: "van_2".into(),
                from: "ref_image_0".into(),
                src_bbox: Some([1, 2, 3, 4]),
                tgt_bbox: None,
                desc: "a van".into(),
            },
        ];
        assert_eq!(
            serde_json::to_string(&rows).unwrap(),
            r#"[{"id":"person_1","bbox":[10,20,300,400],"desc":"a person"},{"id":"van_2","from":"ref_image_0","src_bbox":[1,2,3,4],"tgt_bbox":null,"desc":"a van"}]"#
        );
    }

    #[test]
    fn upsampling_input_is_byte_compatible() {
        let s = upsampling_input("make the lamp red", &[t2i("lamp_1", [1, 2, 3, 4], "d")]);
        assert_eq!(
            s,
            "ORIGINAL PROMPT:\nmake the lamp red\n\nLOCKED BOUNDING-BOX ANNOTATIONS ([y_min, x_min, y_max, x_max] on a 0-1000 canvas, with (0, 0) at the top-left). This is an authoritative JSON list of dictionaries; preserve every supplied identity, provenance value, null, and coordinate exactly:\n[{\"id\":\"lamp_1\",\"bbox\":[1,2,3,4],\"desc\":\"d\"}]"
        );
    }

    #[test]
    fn iou_handles_disjoint_and_equal() {
        assert_eq!(iou(&[0, 0, 10, 10], &[0, 0, 10, 10]), 1.0);
        assert_eq!(iou(&[0, 0, 10, 10], &[20, 20, 30, 30]), 0.0);
        assert!((iou(&[0, 0, 10, 10], &[5, 5, 15, 15]) - 1.0 / 7.0).abs() < 1e-9);
        // Two boxes sharing an edge have zero area intersection.
        assert_eq!(iou(&[0, 0, 10, 10], &[10, 0, 20, 10]), 0.0);
    }

    #[test]
    fn reconcile_renames_a_replanned_locked_box() {
        let locked = vec![t2i("obj_1", [100, 100, 200, 200], "the original desc")];
        // The upsampler re-planned the same box under its own id and cited it.
        let answer = "a lantern <balloon_1> on a table."
            .to_string()
            + " "
            + &serde_json::to_string(&[t2i("balloon_1", [100, 100, 200, 200], "a nice lantern")]).unwrap();
        let out = reconcile(&answer, &locked);
        assert!(
            out.starts_with("a lantern <obj_1> on a table."),
            "the caption citation follows the rename: {out}"
        );
        assert!(
            out.contains(
                r#"{"id":"obj_1","bbox":[100,100,200,200],"desc":"a nice lantern"}"#
            ),
            "the locked id and box, the upsampler's desc: {out}"
        );
    }

    #[test]
    fn reconcile_keeps_locked_verbatim_and_appends_valid_rows() {
        let locked = vec![
            t2i("a_1", [0, 0, 100, 100], "the old a"),
            t2i("b_2", [500, 500, 600, 600], "the old b"),
        ];
        // The answer keeps a_1 (new desc), drops b_2, adds c_3 (cited) and d_4 (not cited).
        let rows = serde_json::json!([
            {"id": "a_1", "bbox": [0, 0, 100, 100], "desc": "a shiny a"},
            {"id": "c_3", "bbox": [300, 300, 350, 350], "desc": "a c"},
            {"id": "d_4", "bbox": [700, 700, 750, 750], "desc": "a d"},
        ]);
        let answer = format!(
            "an a <a_1>, a c <c_3> and an uncited d. {}",
            serde_json::to_string(&rows).unwrap()
        );
        let out = reconcile(&answer, &locked);
        let expected = format!(
            "an a <a_1>, a c <c_3> and an uncited d. {}",
            serde_json::to_string(&vec![
                t2i("a_1", [0, 0, 100, 100], "a shiny a"),
                t2i("b_2", [500, 500, 600, 600], "the old b"),
                t2i("c_3", [300, 300, 350, 350], "a c"),
            ])
            .unwrap()
        );
        assert_eq!(out, expected);
    }

    #[test]
    fn reconcile_with_no_table_in_the_answer_keeps_the_locked_rows() {
        let locked = vec![t2i("a_1", [0, 0, 100, 100], "an a")];
        let out = reconcile("just words, no table", &locked);
        assert_eq!(
            out,
            format!(
                "just words, no table {}",
                serde_json::to_string(&locked).unwrap()
            )
        );
    }
    #[test]
    fn a_reworded_removal_is_reasserted() {
        let locked = vec![edit_row_fixture("mug_2", "mug", Some([500, 60, 800, 320]), None)];
        let cap = "Change the position of the mug <mug_2> to the right side of the desk, and move the notebook <book_2> to the left side of the desk, while keeping the teapot <teapot_1> in its new position on the right.";
        assert_eq!(
            out_check(cap, &locked),
            "Remove the mug <mug_2> from the scene, and move the notebook <book_2> to the left side of the desk, while keeping the teapot <teapot_1> in its new position on the right."
        );
    }

    #[test]
    fn a_change_phrased_removal_is_rewritten_in_place() {
        // The user's T4: the PUP said "change the background to a dark wall"
        // for a removed object and the model obeyed the prose.
        let locked = vec![edit_row_fixture("obj_2", "", Some([537, 4, 970, 176]), None)];
        let cap = "Change the background <obj_2> to a plain dark wall, move the notebook <obj_6> to the right side of the desk, and keep the teapot <obj_7> in place.";
        assert_eq!(
            out_check(cap, &locked),
            "Remove the object <obj_2> from the scene, move the notebook <obj_6> to the right side of the desk, and keep the teapot <obj_7> in place."
        );
    }

    #[test]
    fn a_clause_shared_with_other_ids_is_appended_not_rewritten() {
        let locked = vec![
            edit_row_fixture("mug_2", "mug", Some([500, 60, 800, 320]), None),
            edit_row_fixture("book_2", "book", Some([450, 620, 800, 900]), Some([500, 350, 750, 600])),
        ];
        let cap = "Keep the mug <mug_2> next to the book <book_2> on the shelf.";
        assert_eq!(
            out_check(cap, &locked),
            "Keep the mug <mug_2> next to the book <book_2> on the shelf. Remove the mug <mug_2> from the scene."
        );
    }

    fn out_check(cap: &str, locked: &[BoxRow]) -> String {
        crate::bbox::fix_remove_actions(cap, locked)
    }

    #[test]
    fn a_caption_that_already_says_gone_is_untouched() {
        let locked = vec![edit_row_fixture("mug_2", "mug", Some([500, 60, 800, 320]), None)];
        let cap = "Remove the mug <mug_2> from the desk.";
        assert_eq!(fix_remove_actions(cap, &locked), cap);
        let cap2 = "The mug <mug_2> is no longer on the desk.";
        assert_eq!(fix_remove_actions(cap2, &locked), cap2);
    }

    #[test]
    fn without_locked_removals_the_caption_passes_through() {
        let locked = vec![edit_row_fixture("book_2", "book", Some([450, 620, 800, 900]), Some([500, 350, 750, 600]))];
        let cap = "Move the book <book_2> to the left side of the desk.";
        assert_eq!(fix_remove_actions(cap, &locked), cap);
    }

    #[test]
    fn an_empty_description_falls_back_to_the_object_word() {
        let locked = vec![edit_row_fixture("cup_1", "", Some([100, 100, 300, 300]), None)];
        assert_eq!(fix_remove_actions("A desk.", &locked), "A desk. Remove the object <cup_1> from the scene.");
    }

    #[test]
    fn an_added_row_recovering_a_locked_element_is_dropped() {
        // The T7 case: the upsampler annotates the same element a locked row
        // already owns (a tighter box around the same object).
        let locked = vec![edit_row_fixture("obj_1", "", Some([50, 100, 950, 500]), Some([50, 100, 950, 500]))];
        let rows = vec![
            json!({"id": "obj_1", "from": "ref_image_0", "src_bbox": [50, 100, 950, 500], "tgt_bbox": [50, 100, 950, 500], "desc": ""}),
            json!({"id": "lantern_1", "from": "ref_image_1", "src_bbox": [200, 150, 900, 450], "tgt_bbox": [200, 150, 900, 450], "desc": "a brass lantern"}),
            json!({"id": "window_1", "from": "ref_image_1", "src_bbox": [10, 10, 200, 300], "tgt_bbox": [10, 10, 200, 300], "desc": "a window"}),
        ];
        let text = format!(
            "A scene. <obj_1> <lantern_1> <window_1> {}
",
            serde_json::to_string(&rows).unwrap()
        );
        let out = reconcile(&text, &locked);
        let i = out.rfind("[{").unwrap_or(0);
        let table = &out[i..];
        assert!(table.contains("\"obj_1\""));
        assert!(!table.contains("lantern_1"));
        assert!(table.contains("window_1"));
    }
}

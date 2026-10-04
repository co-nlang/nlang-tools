//! Local savepoint (○) store.
//!
//! Identity is a locally minted random id, never a CAID (`commit.md`
//! §1.5.3) and never `ids.len()+1`. The covering relation is `parents:`
//! on the frame (D50). A commit is an annotation on a circle (D52/D54),
//! not a node and not a CAID. The directory is not CAS — putting ○ in
//! `objects/` would move the object-count of an all-solid universe
//! (`x: 0` has 3 objects).
//!
//! These files survive commit: D43 requires every ○ to already be durable.

use crate::store_codec::{
    decode_staged, encode_savepoint, parse_savepoint_ancestor, parse_savepoint_commit,
    parse_savepoint_parents, parse_savepoint_point, savepoint_combo_text,
};
use crate::value::{ComboVal, ContentHash};
use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const DIR: &str = "savepoints";

fn cannot_read_savepoints(err: &io::Error) -> anyhow::Error {
    anyhow::anyhow!(
        "cannot read .oo/savepoints: {}",
        crate::operator_io_reason(err)
    )
}

fn cannot_write_savepoints(err: &io::Error) -> anyhow::Error {
    anyhow::anyhow!(
        "cannot write .oo/savepoints: {}",
        crate::operator_io_reason(err)
    )
}

fn dir(base: &Path) -> PathBuf {
    base.join(".oo").join(DIR)
}

fn paths(base: &Path) -> Result<Vec<PathBuf>> {
    let d = dir(base);
    let rd = match fs::read_dir(&d) {
        Ok(rd) => rd,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(cannot_read_savepoints(&e)),
    };
    let mut out = Vec::new();
    for e in rd {
        let e = e.map_err(|err| cannot_read_savepoints(&err))?;
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name == "LOG" || name.starts_with('.') {
            continue;
        }
        out.push(p);
    }
    out.sort();
    Ok(out)
}

pub struct Circle {
    pub parents: Vec<String>,
    pub combo: String,
    /// 64-hex digest of the commit this circle became, if any (D52).
    pub commit_digest: Option<String>,
    /// Predecessor commit: 64-hex digest (A3), or a Repair-2 circle
    /// local id. Annotation, not a covering edge.
    pub ancestor: Option<String>,
    /// Commit this circle stood on (D80). `None` means the line is absent:
    /// there was no point, or the declaration did not record one.
    pub point: Option<String>,
}

fn is_legacy_counter_id(id: &str) -> bool {
    id.len() == 16 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Load every circle. Files with no `parents:` line are v0.38/v0.39
/// counter-named bodies: reconstruct a chain along sorted 16-hex ids so
/// the first new ○ does not claim an N-way merge that never happened.
pub fn load_circles(base: &Path) -> Result<BTreeMap<String, Circle>> {
    let files = paths(base)?;
    let mut parsed: BTreeMap<
        String,
        (Option<Vec<String>>, String, Option<String>, Option<String>, Option<String>),
    > = BTreeMap::new();
    for p in &files {
        let id = p
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("savepoint: unreadable name"))?
            .to_string();
        let text = match fs::read_to_string(p) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                continue;
            }
            Err(e) => return Err(cannot_read_savepoints(&e)),
        };
        // An observation ○ is a leaf on the context it stood on (D91).
        // It is not a tip, not a commit, and not a predecessor of the
        // next injection. Readers of the main line never see it.
        if observation_frame(&text) {
            continue;
        }
        let parents = parse_savepoint_parents(&text);
        let combo = savepoint_combo_text(&text).to_string();
        let commit = parse_savepoint_commit(&text);
        let ancestor = parse_savepoint_ancestor(&text);
        let point = parse_savepoint_point(&text);
        parsed.insert(id, (parents, combo, commit, ancestor, point));
    }

    let legacy: Vec<String> = parsed
        .iter()
        .filter(|(id, (p, _, _, _, _))| p.is_none() && is_legacy_counter_id(id))
        .map(|(id, _)| id.clone())
        .collect();

    let mut nodes = BTreeMap::new();
    for (id, (parents, combo, commit_digest, ancestor, point)) in parsed {
        let parents = match parents {
            Some(p) => p,
            None if is_legacy_counter_id(&id) => {
                let i = legacy.iter().position(|x| x == &id).unwrap();
                if i == 0 {
                    Vec::new()
                } else {
                    vec![legacy[i - 1].clone()]
                }
            }
            None => Vec::new(),
        };
        nodes.insert(
            id,
            Circle {
                parents,
                combo,
                commit_digest,
                ancestor,
                point,
            },
        );
    }
    Ok(nodes)
}

/// The store has declared a commit (D81). Either witness is enough: a
/// circle's `commit:` note (D52), or an object this engine can read as a
/// Commit. The kind is the frame the engine wrote (`#nlang/store commit`)
/// or, for the JSON era, `Commit`'s own decoder — never the object's text.
/// Callers ask only when HEAD is absent, so a present point pays nothing
/// here. Reading this does not create directories or files.
pub fn records_a_commit(base: &Path) -> Result<bool> {
    if load_circles(base)?
        .values()
        .any(|n| n.commit_digest.is_some())
    {
        return Ok(true);
    }
    cas_declares_a_commit(base)
}

fn cannot_read_objects(err: &io::Error) -> anyhow::Error {
    anyhow::anyhow!(
        "cannot read store objects: {}",
        crate::operator_io_reason(err)
    )
}

/// Walk `.oo/objects/sha256/` without creating it. `NotFound` is no
/// objects. Any other IO error is an unopenable store, not an empty one
/// (REAL_03 §6.6).
fn cas_declares_a_commit(base: &Path) -> Result<bool> {
    let sha = base.join(".oo").join("objects").join("sha256");
    let rd = match fs::read_dir(&sha) {
        Ok(rd) => rd,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(cannot_read_objects(&e)),
    };
    for bucket in rd {
        let bucket = bucket.map_err(|e| cannot_read_objects(&e))?;
        let kind = bucket.file_type().map_err(|e| cannot_read_objects(&e))?;
        if !kind.is_dir() {
            continue;
        }
        let files = fs::read_dir(bucket.path()).map_err(|e| cannot_read_objects(&e))?;
        for file in files {
            let file = file.map_err(|e| cannot_read_objects(&e))?;
            let kind = file.file_type().map_err(|e| cannot_read_objects(&e))?;
            if !kind.is_file() {
                continue;
            }
            let bytes = match fs::read(file.path()) {
                Ok(b) => b,
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => return Err(cannot_read_objects(&e)),
            };
            if object_declares_commit(&bytes)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Framed era: the kind is the bytes after `#nlang/store`. Only ` commit`
/// is a commit; a value whose text contains that phrase is still a value.
/// A commit frame that does not decode is unopenable, not absent.
/// JSON era: `Commit`'s decoder. A failure there is not a commit.
fn object_declares_commit(bytes: &[u8]) -> Result<bool> {
    let text = match std::str::from_utf8(bytes) {
        Ok(t) => t,
        Err(_) => {
            if bytes.starts_with(FRAME_COMMIT) {
                anyhow::bail!("cannot read store object: commit frame does not decode");
            }
            return Ok(false);
        }
    };
    let rest = text.trim_start();
    if let Some(after) = rest.strip_prefix(crate::store_codec::FRAME) {
        if after.starts_with(" commit") {
            if crate::store_codec::decode_commit(text).is_err() {
                anyhow::bail!("cannot read store object: commit frame does not decode");
            }
            return Ok(true);
        }
        return Ok(false);
    }
    Ok(serde_json::from_str::<crate::value::Commit>(text).is_ok())
}

const FRAME_COMMIT: &[u8] = b"#nlang/store commit";

/// HEAD is the point. Absent while the store has declared a commit is a
/// lost context, not an empty one (D79, D81). The way back is rollback.
/// The sentence does not name a witness the store may not have.
pub const LOST_CONTEXT: &str = "lost context: HEAD is absent and the store records a commit; restore it with rollback <commit> --grant rollback";

fn tips_of(nodes: &BTreeMap<String, Circle>) -> Vec<String> {
    let mentioned: BTreeSet<&str> = nodes
        .values()
        .flat_map(|n| n.parents.iter().map(|s| s.as_str()))
        .collect();
    nodes
        .keys()
        .filter(|id| !mentioned.contains(id.as_str()))
        .cloned()
        .collect()
}

fn write_circle(base: &Path, body: &str) -> Result<String> {
    let d = dir(base);
    fs::create_dir_all(&d).map_err(|e| cannot_write_savepoints(&e))?;
    let leftover = d.join("LOG");
    if leftover.exists() {
        let _ = fs::remove_file(&leftover);
    }
    for _ in 0..8 {
        let id = crate::injections::mint_id()?;
        let dest = d.join(&id);
        if dest.exists() {
            continue;
        }
        crate::storage::atomic_write(&dest, body.as_bytes())?;
        return Ok(id);
    }
    anyhow::bail!("savepoint id: exhausted unique names")
}

/// The unique tip's recorded proposal text and its `point:` digest, if
/// this injection would cover exactly one circle. `None` when there is
/// no tip or more than one (a confluence still mints).
pub fn sole_tip(base: &Path) -> Result<Option<(String, Option<String>)>> {
    let nodes = load_circles(base)?;
    let tips = tips_of(&nodes);
    if !nodes.is_empty() && tips.is_empty() {
        anyhow::bail!("savepoint cycle: ids nonempty and tips empty");
    }
    if tips.len() != 1 {
        return Ok(None);
    }
    let t = nodes.get(&tips[0]).expect("tip id is a loaded circle");
    Ok(Some((t.combo.clone(), t.point.clone())))
}

/// Append a savepoint of `combo` unless it adds nothing.
///
/// (c) / D51: one tip T whose recorded proposal text equals this one.
/// Two or more tips still mint. (a) / D80 ②: `positions_equal` — the
/// current HEAD's root ⊓ the working set before this injection, and the
/// same root ⊓ the working set after it, have the same CAID. T's `point:`
/// is not that comparison.
pub fn record(base: &Path, combo: &ComboVal, positions_equal: bool) -> Result<Option<String>> {
    let nodes = load_circles(base)?;
    let mut tips = tips_of(&nodes);
    if !nodes.is_empty() && tips.is_empty() {
        anyhow::bail!("savepoint cycle: ids nonempty and tips empty");
    }
    tips.sort();
    let candidate_combo = encode_savepoint(combo, &[] as &[String], None, None, None);
    let candidate_combo = savepoint_combo_text(&candidate_combo).to_string();
    if tips.len() == 1 {
        if let Some(t) = nodes.get(&tips[0]) {
            if t.combo == candidate_combo {
                return Ok(None);
            }
        }
        if positions_equal {
            return Ok(None);
        }
    }
    let point = point_to_write(base)?;
    let body = encode_savepoint(combo, &tips, None, None, point.as_deref());
    Ok(Some(write_circle(base, &body)?))
}

/// Frame marker of an observation ○. Only lines before the combo count,
/// so a field whose text is `observation:` stays a main-line circle.
fn observation_frame(text: &str) -> bool {
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('{') {
            return false;
        }
        if line == "observation:" {
            return true;
        }
    }
    false
}

fn escape_frame(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out
}

fn unescape_frame(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

struct ObservationBody {
    point: Option<String>,
    question: String,
    answer: String,
}

fn parse_observation(text: &str) -> Option<ObservationBody> {
    if !observation_frame(text) {
        return None;
    }
    let mut point = None;
    let mut question = None;
    let mut answer = None;
    for line in text.lines() {
        let line = line.trim_start();
        if line.starts_with('{') {
            break;
        }
        if let Some(rest) = line.strip_prefix("point:") {
            let rest = rest.trim();
            if !rest.is_empty() {
                point = Some(rest.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("question:") {
            question = Some(unescape_frame(rest.strip_prefix(' ').unwrap_or(rest)));
        } else if let Some(rest) = line.strip_prefix("answer:") {
            answer = Some(unescape_frame(rest.strip_prefix(' ').unwrap_or(rest)));
        }
    }
    Some(ObservationBody {
        point,
        question: question?,
        answer: answer?,
    })
}

fn encode_observation(
    parents: &[String],
    point: Option<&str>,
    question: &str,
    answer: &str,
) -> String {
    let raw = encode_savepoint(&ComboVal::default(), parents, None, None, point);
    let split = raw.rfind("\n{").unwrap_or(raw.len());
    let (head, combo) = raw.split_at(split);
    format!(
        "{head}\nobservation:\nquestion: {}\nanswer: {}{combo}",
        escape_frame(question),
        escape_frame(answer)
    )
}

/// This declaration receives observation savepoints. Absent `.oo/format`
/// does not (a scratch with no store). An unreadable declaration refuses.
fn records_observations(base: &Path) -> Result<bool> {
    let format = base.join(".oo").join("format");
    match format.try_exists() {
        Ok(false) => Ok(false),
        Ok(true) => {
            let declaration = crate::storage::read_layout_declaration(base)?;
            Ok(crate::storage::layout_records_observations(&declaration))
        }
        Err(e) => Err(anyhow::anyhow!(
            "cannot read `.oo/format`: {}",
            crate::operator_io_reason(&e)
        )),
    }
}

/// Write one observation ○ unless this declaration does not record them,
/// or the same point, question, and answer are already on disk.
///
/// The predecessor set is the main line's current tips. The new file is
/// not itself a tip (`load_circles` skips it), so a later injection still
/// stands on those tips.
///
/// Two writers can both pass the scan and both mint. The extra file is a
/// second leaf with the same triple; the main line does not fork.
pub fn record_observation(base: &Path, question: &str, answer: &str) -> Result<Option<String>> {
    if !records_observations(base)? {
        return Ok(None);
    }
    let point = point_to_write(base)?;
    let nodes = load_circles(base)?;
    if !nodes.is_empty() && tips_of(&nodes).is_empty() {
        anyhow::bail!("savepoint cycle: ids nonempty and tips empty");
    }
    let mut tips = tips_of(&nodes);
    tips.sort();
    for path in paths(base)? {
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(cannot_read_savepoints(&e)),
        };
        let Some(existing) = parse_observation(&text) else {
            continue;
        };
        if existing.point == point && existing.question == question && existing.answer == answer {
            return Ok(None);
        }
    }
    let body = encode_observation(&tips, point.as_deref(), question, answer);
    Ok(Some(write_circle(base, &body)?))
}

/// Whether this declaration writes `point:`. Absent `.oo/format` is the
/// old form (a unit-test scratch). An unreadable declaration refuses.
fn records_point(base: &Path) -> Result<bool> {
    let format = base.join(".oo").join("format");
    match format.try_exists() {
        Ok(false) => Ok(false),
        Ok(true) => {
            let declaration = crate::storage::read_layout_declaration(base)?;
            Ok(crate::storage::layout_records_the_point(&declaration))
        }
        Err(e) => Err(anyhow::anyhow!(
            "cannot read `.oo/format`: {}",
            crate::operator_io_reason(&e)
        )),
    }
}

/// `point:` is the HEAD digest, read now (D55: not derived from the ○ graph).
/// Older declarations omit the line. No HEAD omits it too: that is no point.
fn point_to_write(base: &Path) -> Result<Option<String>> {
    if !records_point(base)? {
        return Ok(None);
    }
    let head_path = base.join(".oo").join("HEAD");
    let s = match fs::read_to_string(&head_path) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(anyhow::anyhow!(
                "cannot read .oo/HEAD: {}",
                crate::operator_io_reason(&e)
            ));
        }
    };
    let hash = ContentHash::parse(s.trim()).map_err(|e| anyhow::anyhow!("{:?}", e))?;
    Ok(Some(hex::encode(hash.digest)))
}

/// Mint the commit event's own circle (D51/D52/D55). Always mints. Combo
/// is empty — the working set already lives on the covering parent; an
/// empty combo cannot equal a workset snapshot, so D51 will not skip a
/// later evolve.
///
/// Covering (`parents:`): `covering` if given; otherwise the unique (or
/// lexicographically first) tip. No tip means a root circle with empty
/// `parents:` (unit-test refine / first commit without a disk evolve).
/// G3 forbids *two* covering parents, not zero.
///
/// Ancestor (annotation, not an H1 edge): the predecessor commit's
/// 64-hex digest. Omitted on the first commit. Pre-arc HEADs have no
/// circle, so naming a circle id would dangle and `gc` would sweep
/// history that was still on disk.
pub fn record_commit(
    base: &Path,
    commit: &ContentHash,
    covering: Option<&str>,
    ancestor: Option<&str>,
) -> Result<Option<String>> {
    let nodes = load_circles(base)?;
    if !nodes.is_empty() && tips_of(&nodes).is_empty() {
        anyhow::bail!("savepoint cycle: ids nonempty and tips empty");
    }
    let parents: Vec<String> = if let Some(p) = covering {
        vec![p.to_string()]
    } else {
        let mut tips = tips_of(&nodes);
        tips.sort();
        match tips.len() {
            0 => Vec::new(),
            _ => vec![tips.into_iter().next().unwrap()],
        }
    };
    let digest = hex::encode(&commit.digest);
    // Written before set_head (D84). `point:` is this commit's digest: the
    // place HEAD will name. Reading HEAD here would record the previous point.
    // `commit:` names the event; `point:` names where the context will stand.
    let point = if records_point(base)? {
        Some(digest.clone())
    } else {
        None
    };
    let body = encode_savepoint(
        &ComboVal::default(),
        &parents,
        Some(&digest),
        ancestor,
        point.as_deref(),
    );
    Ok(Some(write_circle(base, &body)?))
}

/// Directory is truth. Built per call; not a durable cache.
pub fn circle_id_for_commit(base: &Path, digest: &str) -> Result<Option<String>> {
    let nodes = load_circles(base)?;
    Ok(nodes
        .into_iter()
        .find(|(_, n)| n.commit_digest.as_deref() == Some(digest))
        .map(|(id, _)| id))
}

/// Previous commit: `Commit.parent` if still set, else the D55 ancestor
/// annotation on this commit's circle. Does not walk `parents:` — that is
/// the time covering, and rollback does not leave a mark on it.
pub fn previous_commit(
    base: &Path,
    commit: &crate::value::Commit,
    digest: &str,
) -> Result<Option<ContentHash>> {
    if commit.parent.is_some() {
        return Ok(commit.parent.clone());
    }
    let nodes = load_circles(base)?;
    Ok(previous_commit_in(&nodes, commit, digest))
}

/// `previous_commit` against circles already loaded. One history walk
/// reads the directory once.
pub fn previous_commit_in(
    nodes: &BTreeMap<String, Circle>,
    commit: &crate::value::Commit,
    digest: &str,
) -> Option<ContentHash> {
    if let Some(p) = &commit.parent {
        return Some(p.clone());
    }
    let start = nodes
        .iter()
        .find(|(_, n)| n.commit_digest.as_deref() == Some(digest))
        .map(|(id, _)| id.clone())?;
    let aid = nodes.get(&start).and_then(|n| n.ancestor.clone())?;
    hash_from_ancestor(nodes, &aid, digest)
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn hash_from_hex(d: &str) -> Option<ContentHash> {
    if !is_hex64(d) {
        return None;
    }
    let bytes = hex::decode(d).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    Some(ContentHash::v1(bytes))
}

/// A3 spelling: `ancestor:` is the predecessor commit's 64-hex digest.
/// Repair-2 spelling: a circle local id; look up that circle's `commit:`.
fn hash_from_ancestor(
    nodes: &BTreeMap<String, Circle>,
    aid: &str,
    current: &str,
) -> Option<ContentHash> {
    if let Some(h) = hash_from_hex(aid) {
        if hex::encode(&h.digest) == current {
            return None;
        }
        return Some(h);
    }
    let d = nodes.get(aid).and_then(|n| n.commit_digest.as_deref())?;
    if d == current {
        return None;
    }
    hash_from_hex(d)
}

/// Whether `base` is reachable from `head` as a commit ancestor (D55).
/// Dual walk: `parent` if set, else the ancestor annotation.
/// Cycles are cut by a visited set on commit digests.
pub fn commit_is_ancestor(
    base_dir: &Path,
    store: &crate::storage::ObjectStore,
    head: &ContentHash,
    base: &ContentHash,
) -> Result<bool> {
    // A 64-hex note re-enters as v1; the CLI's base is the v2 address.
    // Same commit means the same digest.
    if head.digest == base.digest {
        return Ok(false);
    }
    let mut seen: HashSet<String> = HashSet::new();
    let mut stack = vec![head.clone()];
    while let Some(h) = stack.pop() {
        let d = hex::encode(&h.digest);
        if !seen.insert(d.clone()) {
            continue;
        }
        if h.digest == base.digest {
            return Ok(true);
        }
        let (_resolved, commit) = store.open_commit(&h)?;
        if let Some(p) = previous_commit(base_dir, &commit, &d)? {
            stack.push(p);
        }
    }
    Ok(false)
}

#[allow(dead_code)]
pub fn load(base: &Path, id: &str) -> Result<ComboVal> {
    let text = match fs::read_to_string(dir(base).join(id)) {
        Ok(t) => t,
        Err(e) => return Err(cannot_read_savepoints(&e)),
    };
    decode_staged(&text)
}

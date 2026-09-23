// A type you could not carry.
// Ruling: nlang-spec/meta/oo/STATUS.md D68 (Q-033's D1, 乙: a type name is
//         a single value).
// Order:  nlang-tools/docs/a_type_you_could_not_carry_handover.md
// Recon:  nlang-tools/docs/a_root_only_one_engine_can_build_recon.md 10.3, 10.8
//
// -- What this arc is ----------------------------------------------------
//
// `@list` looks like one name and behaves like two things, and which one
// you get depends on how you spell the expression rather than on the
// value:
//
//     @list.%fmap      the standard root's node      {{ %builtin: … }}
//     (@list).%fmap    _
//     t: @list
//     t.%fmap          _                             (bound: the node is gone)
//     m.t.%fmap        _                             (through a combo: gone)
//
// Measured, the door is purely lexical: `@name.` is resolved against the
// standard root at projection time, and the VALUE of `@name` is always the
// constraint marker. Nothing you can bind ever carries the node.
//
// And the lexical form shadows the marker rather than extending it. For
// the three names that have a standard-root node it reaches further; for
// every other name it reaches nothing at all, including fields the marker
// plainly has:
//
//     @list.%name   "list"      (@list).%name   "list"
//     @int.%name    _           (@int).%name    "int"
//     @zzz.%name    _           (@zzz).%name    "zzz"
//
// SPEC_05 3.2's own example is written `(@int).%name`, the spelling that
// works.
//
// D68 takes 乙: `@X` is ONE value carrying both the marker's fields and the
// standard-root node's, and `@X.f` is ordinary field access.
//
// -- Not an epoch --------------------------------------------------------
//
// %super is a derived reflection field (SPEC_05 3.2, ruling R1) taking the
// direct parent from SPEC_09 2.1. It appears in the standard root zero
// times and does not need to be stored, so merging moves no byte of the
// standard root. G3 pins that.
//
// NOT PROBED, stated so no one mistakes silence for coverage:
//   * Whether `@zzz` should be refused instead of open. D68 reads 乙 as
//     leaving it open (a name with no node is still a type value, just
//     without %fmap), but records that as a consequence rather than a
//     separate ruling. If the delivery finds it cannot stand, say so in
//     N.3 rather than changing it.
//   * D3' (which text form is the normative manifest) and D4. Both are
//     open, and D3' has a prerequisite of its own -- REAL_03 6.2's value
//     tag table is missing five tags, including the one the standard root
//     itself needs.

use std::fs;
use std::path::Path;
use std::process::Command;

fn cmd(dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oo"));
    c.current_dir(dir)
        .env("OO_IDENTITY", dir.join("identity-for-tests"))
        .env("OO_NODE_HOME", dir.join("node-home-for-tests"));
    c
}

fn eval(dir: &Path, expr: &str) -> String {
    let o = cmd(dir).args(["eval", expr]).output().expect("oo runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
    .trim()
    .to_string()
}

fn observe(dir: &Path, program: &str, field: &str) -> String {
    fs::write(dir.join("obs.n"), program).expect("write obs.n");
    let o = cmd(dir)
        .args(["run", "obs.n", "--observe", field])
        .output()
        .expect("oo runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
    .trim()
    .to_string()
}

fn scratch(tag: &str) -> nlang_interpreter::ScratchDir {
    nlang_interpreter::ScratchDir::new(&format!("carry-{tag}"))
}

/// Every type name this arc speaks about: the three with a standard-root
/// node, a spread of SPEC_09 2.1's tree, and one nobody minted.
const NAMES: &[&str] = &[
    "list", "option", "result", "int", "num", "str", "bool", "any", "float", "combo", "record",
    "zzz",
];

fn is_absent(s: &str) -> bool {
    let body = s.split(";;").next().unwrap_or("").trim();
    body == "_"
}

// ---------------------------------------------------------------------
// G1. The control. The parenthesised form answers for every name, today
// and after. If this goes red the arc broke the side that worked.
// ---------------------------------------------------------------------
#[test]
fn g1_the_marker_answers_for_every_name() {
    let s = scratch("g1");
    let d = s.path();
    for n in NAMES {
        let got = eval(d, &format!("((@{n}).%name)"));
        assert!(
            got.contains(&format!("\"{n}\"")),
            "(@{n}).%name must answer \"{n}\": {got}"
        );
    }
}

// ---------------------------------------------------------------------
// G2. RED LINE. %super keeps doing exactly what SPEC_05 3.2 and ruling R1
// say: derived from SPEC_09 2.1's tree, @any has no parent, the chain
// composes. The arc must not move it.
// ---------------------------------------------------------------------
#[test]
fn g2_super_still_reflects_the_type_tree() {
    let s = scratch("g2");
    let d = s.path();
    let up = eval(d, "((@int).%super.%name)");
    assert!(
        up.contains("\"num\""),
        "SPEC_09 2.1 makes @num the direct parent of @int: {up}"
    );
    let top = eval(d, "((@any).%super)");
    assert!(
        is_absent(&top),
        "SPEC_05 3.2: the universal type has no parent and the field is \
         honestly open: {top}"
    );
    // %super is attached to a type value, not computed from fields: a
    // hand-made combo wearing the same fields must not acquire it.
    let fake = eval(d, "(({ %kind: #type, %name: \"int\" }).%super)");
    assert!(
        is_absent(&fake),
        "a plain combo that merely looks like a type must not get %super: \
         {fake}"
    );
}

// ---------------------------------------------------------------------
// G3. RED LINE. Not an epoch. %super is derived and appears in the
// standard root zero times, so nothing here may move an address.
// ---------------------------------------------------------------------
#[test]
fn g3_no_address_moves() {
    let s = scratch("g3");
    let d = s.path();
    fs::write(d.join("a.n"), "x: 0\n").expect("source");
    let o = cmd(d).args(["evolve", "a.n"]).output().expect("evolve");
    assert!(o.status.success(), "REACH: evolve");
    let o = cmd(d).args(["commit", "-m", "r"]).output().expect("commit");
    assert!(o.status.success(), "REACH: commit");

    let out = cmd(d).args(["status"]).output().expect("status");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        text.contains("7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911"),
        "the standard root digest moved: {text}"
    );
    let objects = fs::read_dir(d.join(".oo/objects/sha256"))
        .expect("objects")
        .flatten()
        .filter_map(|e| fs::read_dir(e.path()).ok())
        .flat_map(|rd| rd.flatten())
        .count();
    assert_eq!(objects, 3, "object count moved");
    assert_eq!(
        fs::read_to_string(d.join(".oo/format")).unwrap_or_default(),
        "layout=6\n"
    );
    assert_eq!(
        fs::read_to_string(d.join(".oo/objects.format")).unwrap_or_default(),
        "encoding=5\n"
    );
}

// ---------------------------------------------------------------------
// R1. THE FIRST. One name, one answer: the two spellings must agree.
// ---------------------------------------------------------------------
#[test]
fn r1_both_spellings_of_a_type_name_answer_the_same() {
    let s = scratch("r1");
    let d = s.path();
    let mut disagree = Vec::new();
    for n in NAMES {
        for f in ["%name", "%kind"] {
            let bare = eval(d, &format!("(@{n}.{f})"));
            let paren = eval(d, &format!("((@{n}).{f})"));
            if bare != paren {
                disagree.push(format!("  @{n}.{f}: bare {bare:?} vs paren {paren:?}"));
            }
        }
    }
    assert!(
        disagree.is_empty(),
        "{} spellings disagree. `@X.f` is not field access on `@X`; it is a \
         lookup against the standard root that SHADOWS the marker, so for \
         every name without a node it withholds fields the marker plainly \
         has. D68 says one name is one value.\n{}",
        disagree.len(),
        disagree.join("\n")
    );
}

// ---------------------------------------------------------------------
// R2. THE SECOND. A type is a value you can carry. Bind it, put it in a
// combo, and it is still the same thing.
// ---------------------------------------------------------------------
#[test]
fn r2_a_type_survives_being_bound() {
    let s = scratch("r2");
    let d = s.path();

    // Control: the literal reaches the node, so the fixture is live.
    let direct = eval(d, "(@list.%fmap)");
    assert!(
        !is_absent(&direct),
        "CONTROL: the literal spelling must reach the node: {direct}"
    );

    let bound = observe(d, "t: @list\nout: (t.%fmap)\n", "out");
    assert!(
        !is_absent(&bound),
        "binding a type to a name loses it: `t: @list` and then `t.%fmap` \
         answers as though nothing is there, while `@list.%fmap` reaches the \
         node. A type is not a value you can carry: {bound}"
    );

    let nested = observe(d, "m: { t: @list }\nout: (m.t.%fmap)\n", "out");
    assert!(
        !is_absent(&nested),
        "putting a type inside a combo loses it too: {nested}"
    );
}

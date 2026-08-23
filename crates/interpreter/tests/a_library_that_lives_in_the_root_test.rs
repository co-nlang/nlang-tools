//! Q-035 repair 1: a universe whose library lives IN its root must still
//! dispatch.
//!
//! `universe.rs:159` `standard_for_root` returns an EMPTY table when the root
//! names no standard-root digest, with the comment "Formats 1/2 were
//! self-contained; keeping the standard layer empty preserves that shape".
//! Self-contained means the library is in the root. Before the Q3.B gate that
//! was harmless, because dispatch never consulted the table. After the gate an
//! installed-but-empty table projects nothing, so every `%builtin` in such a
//! universe -- including the standard library's own -- collapses.
//!
//! Measured 2026-08-23 with three real binaries on ONE pre-digest repo
//! (`/home/gali/nlang/.oo`, HEAD 2026-08-14, 67,494 B root, copied; the
//! original was not touched):
//!
//!   v0.20.0 (which wrote it)   lib: 7    out: 3
//!   pre-delivery  ebc0a5a      lib: 7    out: 3
//!   delivery      4d047f4      lib: BOTTOM #unprojected_builtin
//!                              out: BOTTOM #unprojected_builtin
//!
//! `lib` is `~%Math./add (3,4)` -- the LEGITIMATE library call, not a forgery.
//! The history still opens and the names still resolve; nothing computes. That
//! is the heavy half of REAL_03 §6.8.1's distinction.
//!
//! The fix is not to drop the gate. A pre-digest root carries the library
//! under its `system` axis (measured: 238 of 256 `builtin` occurrences under
//! `system`, 0 under `data`), so the credential for such a universe is the
//! `~%` axis of the table it actually has -- inline instead of a separate CAS
//! object. That keeps O68 Q3.B ("the credential is this universe's table")
//! and keeps a forged `%builtin` in `data` exactly as (un)authorised as it is
//! under format 3.

use nlang_interpreter::{ComboVal, Ouroboros, Universe};
use nlang_parser::parse_program;

/// The shape `Universe::load` produces for a root with no standard digest:
/// library hydrated into the user root, standard layer empty.
fn legacy_universe(oo: &Ouroboros) -> Universe {
    Universe::new_with_standard(None, oo.root_with_system(), ComboVal::default())
}

fn evolve_and_read(oo: &Ouroboros, u: &mut Universe, src: &str, coord: &str) -> String {
    let program = parse_program(src).expect("parse");
    for f in &program.fields {
        let _ = u.evolve(oo, f);
    }
    u.staged
        .get_field(coord)
        .map(|v| v.to_nlang(0))
        .unwrap_or_else(|| "<absent>".to_string())
}

#[test]
fn a_legacy_root_still_dispatches_its_own_library() {
    let oo = Ouroboros::new_in_memory();
    let mut u = legacy_universe(&oo);

    // REACH: the name must resolve at all. If this is `<absent>` the test
    // proves nothing about dispatch.
    let got = evolve_and_read(&oo, &mut u, "lib: ~%Math./add (3,4)\n", "lib");
    assert_ne!(got, "<absent>", "REACH: `lib` must exist after evolve");

    assert_eq!(
        got, "7",
        "a universe whose library lives in its own root must still dispatch it; \
         got {got:?}. The standard layer being empty is how formats 1/2 are \
         loaded, not a statement that the universe has no library."
    );
}

#[test]
fn the_gate_still_holds_for_a_name_that_root_does_not_have() {
    // The repair must not become "legacy universes have no gate". A name no
    // table projects must still be refused by name.
    let oo = Ouroboros::new_in_memory();
    let mut u = legacy_universe(&oo);
    let got = evolve_and_read(
        &oo,
        &mut u,
        "bad: {{ %builtin: \"nonexistent.thing\", %morphism: #true }} (6,3)\n",
        "bad",
    );
    assert_ne!(got, "<absent>", "REACH: `bad` must exist after evolve");
    assert!(
        got.contains("#unprojected_builtin"),
        "an invented name must stay refused even in a legacy universe: {got:?}"
    );
}

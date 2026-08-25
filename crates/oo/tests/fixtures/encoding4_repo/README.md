# `encoding4_repo` — a repo written by the last JSON engine

Built 2026-08-26 by the real `oo v0.35.0` binary (`app: { k1: 1 }`, one commit),
which is the last release before Q-012 changes the CAS encoding. `oo_dir/` is
the repo's `.oo/` directory, stored under a different name because `.oo` is in
`.gitignore`. A test copies it into a scratch directory as `.oo`.

Why it is checked in rather than generated: after Q-012 lands, no engine in this
tree writes `encoding=4` any more, so the only way to keep testing that a
pre-Q-012 repo still opens and still migrates is to keep one.

What makes it the real thing:

- `objects.format` declares `encoding=4`, `format` declares `layout=2`.
- The root object is 428 B of `serde` JSON of the Rust `Value` enum
  (`{"Combo":{…}}`, `{"Atom":[…,0,null]}`).
- The standard root object is 136,268 B: `"standard-root:<hex>"`, i.e. 68,126 B
  of JSON hex-encoded inside a JSON string — exactly 2.00x its own content.
- The commit object is not a `Value` at all: `kind`/`meta`/`parent`/`root`, with
  a base64 `lattice_sketch` and the digest as 32 decimal integers.

Its two addresses are the arc's identity red line and must not move:

    root          932a9f9dd62297a7cb3cb9c9fb56907a06a8c4d4e945cc3dfc4782a6987fb0cb
    standard root 7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911

Do not regenerate it with a current engine once Q-012 has landed: a current
engine would write the new encoding, which is the opposite of what this is for.

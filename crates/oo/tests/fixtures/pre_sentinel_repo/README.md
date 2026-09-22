# `pre_sentinel_repo` — a repo created by an engine older than the sentinel

Built 2026-08-25 by the real `oo v0.20.0` binary (`app: { k1: 1 }`, one commit).
`oo_dir/` is the repo's `.oo/` directory, stored under a different name because
`.oo` is in `.gitignore`. A test copies it into a scratch directory as `.oo`.

Why it is checked in rather than generated: the property under test is that a
repo written before `REAL_03` §6.8 (root names its standard root by one digest)
stays openable. Generating one needs an engine that predates the sentinel, and
no such binary ships with this repo. 67,913 of the 68,526 bytes are the root
object, which embeds the whole standard root — that is exactly the shape §6.8
was added to stop, and it is what makes this fixture the real thing rather than
a synthetic one.

Do not regenerate it with a current engine: a current engine writes a root that
names a digest, which is the opposite of what this fixture is for.

## It can be reproduced, and the reproducer is not in this repository (2026-09-22, Q-053)

"No such binary ships with this repo" is true, and it is about the *repo*. It
is often read as "this cannot be rebuilt". On the machine this was made on it
can:

    /home/gali/nlang-baselines/v0.20.0-target/debug/oo   (oo v0.20.0)
    echo 'app: { k1: 1 }' > main.n && oo evolve main.n && oo commit -m "legacy commit"

Measured 2026-09-22: the root object comes out **byte-for-byte identical** --
same 67,913 B, same digest `16ba5683…`, `sha256sum` of the file `95dd69bb…` on
both sides. Only the commit object differs, by its timestamp. So the artifact
is reproducible, while `/home/gali/nlang-baselines/` is inside no git
repository: **the checked-in bytes are the durable copy, not the binary.**

Since `oo v0.56.0` (the `Thunk` identity epoch) this engine can no longer
verify the root: `#caid_mismatch`, requested `16ba5683…` vs recomputed
`cef5e484…` (ruling D72). The store is still *parsed* -- `status` answers
`Standard root dependency: self-contained (pre-sentinel)` -- so what this
fixture witnesses about `REAL_03` §6.8 does not need the digest to verify, and
does not need any binary at all:

  * `tests/evidence_that_needs_a_binary_probe_test.rs` reads these bytes as
    data and asserts the §6.8 facts directly (the root is 99.1% of the store;
    no `masa_ref` anywhere is a `Digest`; the value kinds it contains are the
    closed list `Combo` / `Thunk` / `Atom{Int,Str,Tag}`, which is the blast
    radius of the next identity epoch). **No epoch can reach those.**
  * The one epoch-sensitive number, `cef5e484`, is pinned alone in that file's
    `e5`, so that when an epoch moves it the failing test name says so.

Do not regenerate it into this directory. The point of a checked-in artifact is
that it is the same bytes for every reader, including readers who do not have a
v0.20.0 binary -- which, outside this machine, is everyone.

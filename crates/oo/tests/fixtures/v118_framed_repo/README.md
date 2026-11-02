# `v118_framed_repo` — records whose fields read names, two commits

Built 2026-11-02 by the real `oo v0.118.0` binary (tag build at
`/home/gali/nlang-baselines/v0.118.0-verify`, not in version control). `oo_dir/`
is the repo's `.oo/`, stored under another name because `.oo` is ignored.

    oo evolve main.n
    oo commit -m base           root …:8e4996a6155d482b129cd9a68b2e3ab21cdb2473ed9e31f2301f1311ffdcbdbe
    oo evolve facet.n
    oo commit -m facet

    format          layout=9
    objects.format  encoding=10
    commit (HEAD)   hash:sha256:v2:…:66fb423d3229ab91857606c18b7022fa60a8c87f8036bc9924d9ac19c3a2e120
    root            hash:sha256:v2:…:2b1c12b3b8d3dd4a4f7f56294e0986b69bc092ed7050e5b3c6d97fd886d5d30a

`facet.n` holds a hidden type (`~@T`) beside a field that reads a field.
Without it, the encoding-9 rule (hidden facets on the local axis, frames
written out) computes the same address as the encoding-10 rule, and a read
that skips the encoding-10 rule is rescued by the encoding-9 one.

Why it exists: every record here holds a field that is not forced yet, a
thunk whose closure frame is the record. v0.118.0 writes that frame out in
full in the thunk's identity; D126 (Q-118) writes its digest. A store
declaring encoding 10 keeps the v0.118.0 rule. The probe
(`a_record_carried_in_every_field_probe_test`, r3) writes into a copy of
this store and pins the root `oo v0.118.0` itself writes there:

    z: 3
    h2: { a: 7, b: a + z }
    h: { c: b * 2 }

    root            …:45f1d436d364f9237184252c7b667c51cf6ab422c9a00c12d0418c2db421cae9

and `oo v0.118.0` still opens the copy after a current engine has written it.

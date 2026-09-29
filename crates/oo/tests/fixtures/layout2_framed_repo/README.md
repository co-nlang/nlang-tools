# `layout2_framed_repo` — a store older than its savepoints' commit notes

Built 2026-09-29 by the real `oo v0.40.0` binary (tag build at
`/home/gali/nlang-baselines/v0.40.0-verify`, not in version control), the last
release before D52 gives a commit its own savepoint with a `commit:` note.
`oo_dir/` is the repo's `.oo/`, stored under another name because `.oo` is
ignored. A test copies it into a scratch directory as `.oo`.

    oo evolve a.n && oo commit -m first
    oo evolve b.n && oo commit -m second

    format          layout=2
    objects.format  encoding=5   (framed: a commit object begins `#nlang/store commit`)
    HEAD            hash:sha256:v1:55b12926437694d2aedcec2e96e45cf319fd4682966bcfb4a497067cd83982b7
    first           hash:sha256:v1:84a0ebde30eecb29b7f6b879cb7e21bfa4624b360f2ced74d2221684e39d9fc6
    savepoints      2, neither carries a `commit:` note

Why it exists: D79 recognises a lost context by the savepoints' `commit:`
notes, which this store does not have. Measured on v0.64.0: with `HEAD` moved
aside, `gc --grant gc` says "0 reachable" and deletes 5/5 objects, before and
after `migrate` to `layout=8`. D81 widens the evidence to "the store declares
a commit": a savepoint note, or an object the engine reads as a Commit.

Do not regenerate it with a current engine.

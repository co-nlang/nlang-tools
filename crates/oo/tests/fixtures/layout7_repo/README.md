# `layout7_repo` — a store declared `layout=7`, one commit

Built 2026-09-27 by the real `oo v0.63.0` binary (tag build at
`/home/gali/nlang-baselines/v0.63.0-verify`, not in version control). `oo_dir/`
is the repo's `.oo/`, stored under another name because `.oo` is ignored.

    printf 'a: 1\n' > main.n
    oo evolve main.n
    oo commit -m base

    format          layout=7
    commit (HEAD)   hash:sha256:v2:…:ae2cecdf8c3b507caa868e6c5b8f2f442146a82434c3d4ac9e8a92f2452c752d

Why it exists: D80 makes an injection savepoint record the point it stood on,
which is a field `layout=7` cannot declare (REAL_02 §5.1.1) — so a store
declared `layout=7` keeps receiving the old form until it is explicitly
migrated, and the migrate names the engines it locks out (v0.61.0 … v0.63.0
open layout 7). This fixture is a real layout-7 store to test both.

Do not regenerate it with a current engine.

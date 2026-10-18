# `v093_dotted_functions_repo` — records written field by field, one commit

Built 2026-10-18 by the real `oo v0.93.0` binary (tag build at
`/home/gali/nlang-baselines/v0.93.0-verify`, not in version control). `oo_dir/`
is the repo's `.oo/`, stored under another name because `.oo` is ignored.

    oo evolve main.n
    oo commit -m base

    format          layout=9
    commit (HEAD)   hash:sha256:v2:…:307cfea5153f9cfdeeb965b317cb9f2c30fe9c34bb511e13fb36b42cd2b57a5a
    root            hash:sha256:v2:…:980a6784dce41314c4e040f93f3b9fb26e3095e2e26841c42853bbd6be465376

Why it exists: v0.93.0 keeps the meet of two writes to one record as its
answer. When the record holds a function (`api.f`, `api.g`; `r.z` a call's
function; `s.w` an arrow from a literal) that answer does not print back as
syntax. Q-092 R-5 refused every later write to such a record at commit
(`Commit failed`); R-6 meets it as v0.93.0 did. A current engine cannot
write this store, so the probe (`a_recipe_that_met_a_write_probe_test`, r18)
reads it from here.

Do not regenerate it with a current engine.

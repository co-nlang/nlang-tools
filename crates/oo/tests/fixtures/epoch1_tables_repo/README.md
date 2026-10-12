# `epoch1_tables_repo` — tables stored in identity epoch 1

Built 2026-10-12 by the real `oo v0.84.0` binary (tag build at
`/home/gali/nlang-baselines/v0.84.0-verify`, not in version control), during
Q-083 acceptance. `oo_dir/` is the repo's `.oo/`.

    oo evolve main.n && oo commit -m "epoch one tables"

    format          layout=9
    objects.format  encoding=5
    HEAD            …:cba0ba9fff127c80eac1fb536501c596b4562d768ce494293d5349cbda813565
    standard root   7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911

Why it exists: a table evaluated at the top of a file is stored with its
branches named by digests — `%rules` keys are `{content_digest(pattern)}`,
`%bodies` keys `{content_digest(body)}`. Those names are epoch-1 digests. An
engine that reads them and then names the same pattern with an epoch-2 digest
holds two branches for one constraint (D101). `epoch1_repo` has only a table
one field down, which is stored as code and carries no names — so it could not
see this.

What `oo v0.84.0` answers in it:

    _.k "s"                                   9
    _.k 5                                     1
    _.b "k"                                   { a: 1 b: 2 }
    _.b "z"                                   0
    _.k = { @{ @str }: 9, @{ @int }: 1 }      #true
    (_.k & { @{ @str }: 10 }) "s"             ⊥ #conflict
    (_.b & { @{ "k" }: { c: 3 } }) "k"        { a: 1 b: 2 c: 3 }
    (_.b & { @{ "k" }: { a: 5 } }) "k"        ⊥ #conflict

Do not regenerate it with a current engine.

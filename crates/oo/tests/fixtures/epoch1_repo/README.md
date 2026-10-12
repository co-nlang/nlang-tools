# `epoch1_repo` — a store written in identity epoch 1

Built 2026-10-12 by the real `oo v0.84.0` binary (tag build at
`/home/gali/nlang-baselines/v0.84.0-verify`, not in version control), the last
release whose identity encoding writes no axis (Q-083 / D106). `oo_dir/` is the
repo's `.oo/`, stored under another name because `.oo` is ignored.

    oo evolve main.n   && oo commit -m "epoch one"
    oo evolve second.n && oo commit -m "epoch one, second"

    format          layout=9
    objects.format  encoding=5
    first commit    …:2858af651809a9aa86cddd06558846d326fbb15a196d874a21ca7559c06f93a3
    HEAD            …:f8e94c4719238893d5662b01365f8c3dd0300c2e449ea01504f2b95ca495e7fd
    standard root   7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911

What `oo v0.84.0` answers in it:

    _.t        { int: 1 }
    _.k.t "s"  9
    _.k.t 5    1
    _.f 1      2
    _.u        1 | 2
    _.x        0

`main.n` holds one of each thing the epoch changes the bytes of: a field whose
address collided with another axis (`t: { int: 1 }`), a table one field down,
a morphism, a union, a private field. Every address in it is an epoch-1
address; the engine that reads it must recompute them with the frozen epoch-1
encoding (REAL_03 §6.6 across epochs, D106 乙).

Do not regenerate it with a current engine: a current engine writes epoch-2
addresses, which is the opposite of what this is for.

# `epoch1_encoding3_repo` — a store whose encoding `migrate` does not advance

Built 2026-10-13 by the real `oo v0.25.0` binary (debug build at
`/home/gali/nlang-baselines/v0.25.0-verify-target`, not in version control),
during Q-083 acceptance. `oo_dir/` is the repo's `.oo/`.

    oo evolve main.n && oo commit -m "encoding three"

    format          layout=2
    objects.format  encoding=3
    HEAD            hash:sha256:v1:a733006555c09668cdd080db1aea6df3a142f7f9feeb74d1c8241a6aec80d989
    standard root   65f52e2da48baa550d7340c0fdc214fd1f9925577a96ffec59bc34f8b2bcbe72

Why it exists: `oo migrate` leaves `encoding=3` as it is — the roots in such a
store keep the address rule from before v0.26.0 (`hash(standard ⊕ user)`,
O63). So in this store a write that epoch 1 cannot tell apart (a pin that
changes only an axis) has no way out through `migrate`, and a refusal that
says "run `oo migrate`" sends the operator to do nothing. A store relabelled
`encoding=3` by hand is not this: its root follows the later rule, and
`oo v0.84.0` itself reports it corrupt.

Do not regenerate it with a current engine.

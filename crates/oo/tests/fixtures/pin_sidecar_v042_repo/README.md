# `pin_sidecar_v042_repo` — a pin whose coordinates live in `.oo/pin_pending`

Built 2026-11-05 by the real `oo v0.42.0` binary (tag build at
`/home/gali/nlang-baselines/v0.42.0-verify`, not in version control), the last
release that records a pin's coordinates in the shared sidecar `.oo/pin_pending`
instead of on the member (D58 / D60). `oo_dir/` is the repo's `.oo/`, stored
under another name because `.oo` is ignored.

    main.n  h: { k: 5, r: 6 }      oo evolve main.n && oo commit -m "before the pin"
    pin.n   h: { k: 7 }            oo evolve --pin --grant pin pin.n

    format          layout=3
    objects.format  encoding=5
    HEAD            …:b54cde1acc5df61a668dbdbd17a06c74ebf98c0dfde8057492b67b175cd00b4e
    pin_pending     ["h"]
    member          injections/247ce8f151fa1911da4c14110d45df80  (h: { k: 7 })

Used by `a_pin_that_erased_its_neighbours_probe_test` r6 (Q-124, D128): the
sidecar names the top-level field `h`; the pin writes the leaf `h.k`, so a
commit by this engine keeps `h.r`.

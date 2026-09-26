# `layout6_signed_repo` — a new-form commit whose signature signs only sources and targets

Built 2026-09-26 by the real `oo v0.60.0` binary (tag build at
`/home/gali/nlang-baselines/v0.60.0-verify`, not in version control). `oo_dir/`
is the repo's `.oo/`, stored under another name because `.oo` is ignored.

    printf 'x: 1\n' > main.n
    oo evolve main.n
    oo commit -m base
    oo refine --source <root CAID> --target <root CAID> -m signed --sign

With no `.oo/architects.json` the refine ran under the genesis exemption, so the
stored word is `unverified`; the operator key signed it.

    format          layout=6
    commit (base)   hash:sha256:v2:…:893141dd19ab4eab1f6e347d0ba3ff3c8622ea24ee172b8e9fb4f8ae8da7ed61
    commit (refine) hash:sha256:v2:…:5acf6b6e79b1eb744bba08d0504854f725f8a89c2850d138a61aa5cd96f44574  (HEAD)
    signer          8047eac6051c05302ca7a99433f70bd5e029609f63b08c1f3f0d7d9361bc0b23

Why it exists: v0.59.0 and v0.60.0 put the signature inside the commit's address
(D74), but what the key signed is only `refine:` + the sorted source and target
CAIDs -- not the commit. The same signature verifies in any commit with the same
sources and targets. D76 甲: a reader shows such a signature for what it signs,
distinct from one that signs the commit. D76 ③: a `layout=6` store keeps
receiving this form until it is explicitly migrated. This fixture is the only
real instance of an old-form signature on a new-form commit.

Do not regenerate it with a current engine.

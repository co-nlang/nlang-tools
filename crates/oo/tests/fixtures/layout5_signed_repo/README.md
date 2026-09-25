# `layout5_signed_repo` — a legacy refine that carries a signature

Built 2026-09-25 by the real `oo v0.58.0` binary (tag build at
`/home/gali/nlang-baselines/v0.58.0-verify`, not in version control). `oo_dir/`
is the repo's `.oo/`, stored under another name because `.oo` is ignored.

    printf 'x: 1\n' > main.n
    oo evolve main.n
    oo commit -m base
    oo refine --source <root CAID> --target <root CAID> -m signed --sign

With no `.oo/architects.json` the refine ran under the genesis exemption, so the
stored word is `unverified` -- but the operator key DID sign it, and the
signature is stored beside that word, outside the legacy commit address.

    format          layout=5
    commit (base)   hash:sha256:v1:d9d19b46ab0e292c9bfe99462b12d60b8156bb42b3efc7b6ee1060dbb85cb912
    commit (refine) hash:sha256:v1:f1084c1c3b01662b528bfd947bd2984df26cc94cb77407b8f6401ddb065e73d9  (HEAD)
    signer          79b908e3eb5fc19f5f6645906765ae76e5c2963950c4753c8c9c5b05e6616de4

Why it exists: D75 says the only authority a reader presents is a signature it
re-verifies at read time. On a legacy commit that is a new fact computed by the
reader, not the stored word, so D74 ② does not forbid it. This fixture is the
only way to test that on a commit whose address does not cover the signature.

Do not regenerate it with a current engine.

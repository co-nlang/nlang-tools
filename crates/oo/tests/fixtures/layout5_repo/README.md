# `layout5_repo` — a repo whose commits are addressed the old way

Built 2026-09-24 by the real `oo v0.58.0` binary (tag build at
`/home/gali/nlang-baselines/v0.58.0-verify`, not in version control), which is
the last release before D74 makes a commit's address the address of its n/
value. `oo_dir/` is the repo's `.oo/` directory, stored under a different name
because `.oo` is in `.gitignore`. A test copies it into a scratch directory as
`.oo`.

What was run, in an empty directory:

    printf 'x: 1\n' > main.n
    oo evolve main.n
    oo commit -m base
    oo refine --source <root CAID> --target <root CAID> -m 'refine é'

so it holds one ordinary commit and one refine commit made under the genesis
exemption (`refine authority: unverified`), with a non-ASCII message.

    format          layout=5
    objects.format  encoding=5
    commit (base)   hash:sha256:v1:71385b0a3effe7900d941d154a9a05f96b95b49a1f3d50ba5f91098fe168e30b
    commit (refine) hash:sha256:v1:5e9e7d7b2af3261f4db4e41cede8d22a675d97c6a02f24d6de43e63b1d24fa75  (HEAD)
    root            ca0986d5f6331359f34dd1d24a42a33e9a8a375605b78d444ba82e281bcde33c
    standard root   7038e2504b8ef4d4d267dd23b0989946c84303da34fb7e71d01c5b58caf37911

Why it is checked in rather than generated: once D74 lands, no engine in this
tree creates a store whose commits are addressed by the legacy algorithm, and
existing stores are never rewritten (REAL_02 §5.1.1). The only way to keep
testing that such commits still verify, and that the fields their address does
not cover are not presented as fact, is to keep one.

Do not regenerate it with a current engine.

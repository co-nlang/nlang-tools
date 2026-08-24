# `pre_sentinel_repo` — a repo created by an engine older than the sentinel

Built 2026-08-25 by the real `oo v0.20.0` binary (`app: { k1: 1 }`, one commit).
`oo_dir/` is the repo's `.oo/` directory, stored under a different name because
`.oo` is in `.gitignore`. A test copies it into a scratch directory as `.oo`.

Why it is checked in rather than generated: the property under test is that a
repo written before `REAL_03` §6.8 (root names its standard root by one digest)
stays openable. Generating one needs an engine that predates the sentinel, and
no such binary ships with this repo. 67,913 of the 68,526 bytes are the root
object, which embeds the whole standard root — that is exactly the shape §6.8
was added to stop, and it is what makes this fixture the real thing rather than
a synthetic one.

Do not regenerate it with a current engine: a current engine writes a root that
names a digest, which is the opposite of what this fixture is for.

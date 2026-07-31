// Bind `oo --version` to the git tag: the annotated release tag is the
// single source of truth (release protocol tags the measured commit).
// On the tagged commit `git describe` yields exactly `v0.2.N`; anywhere
// else it yields `v0.2.N-<ahead>-g<hash>` (+ trailing `+` when dirty) —
// honest about not being a release build. Falls back to CARGO_PKG_VERSION
// when git is unavailable (e.g. building from a source tarball).

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn main() {
    // Invalidate the build-script cache when HEAD or tags move, so the
    // embedded version tracks checkout/commit/tag changes. The repo may be
    // a submodule (`.git` is a gitdir pointer file), so resolve the real
    // git dir instead of assuming `../../.git/`.
    if let Some(git_dir) = git(&["rev-parse", "--absolute-git-dir"]) {
        println!("cargo:rerun-if-changed={git_dir}/HEAD");
        println!("cargo:rerun-if-changed={git_dir}/refs/tags");
        println!("cargo:rerun-if-changed={git_dir}/packed-refs");
    }
    let version = git(&["describe", "--tags", "--always", "--dirty=+"])
        .unwrap_or_else(|| format!("v{}", env!("CARGO_PKG_VERSION")));
    println!("cargo:rustc-env=OO_VERSION={version}");
}

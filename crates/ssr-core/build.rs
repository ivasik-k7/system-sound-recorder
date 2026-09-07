//! Stamps the commit into the binary.
//!
//! A recording outlives the build that made it, and "which version produced
//! this file" is the first question of any bug report. The version alone does
//! not answer it — between two tags there are many builds — so the short commit
//! travels with it.

use std::process::Command;

fn main() {
    println!("cargo:rustc-env=SSR_BUILD_REF={}", build_ref());

    // Rebuild when the checked-out commit changes, and no more often than
    // that. Watching the working tree instead would rebuild on every edit for
    // a string that had not changed.
    for path in ["../../.git/HEAD", "../../.git/refs/heads"] {
        if std::path::Path::new(path).exists() {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
}

/// The short commit, however it can be discovered.
fn build_ref() -> String {
    // A checkout is the usual case, including on a build machine.
    if let Some(hash) = git(&["rev-parse", "--short=7", "HEAD"]) {
        return hash;
    }
    // A build from a source archive has no repository, but a CI run that
    // exported the commit still knows it.
    if let Ok(sha) = std::env::var("GITHUB_SHA") {
        if sha.len() >= 7 {
            return sha[..7].to_string();
        }
    }
    // Neither. Say so rather than inventing something that looks like a
    // commit and is not one.
    "unknown".to_string()
}

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

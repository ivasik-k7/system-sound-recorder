//! What this build calls itself.
//!
//! One string, in one place, so the command line, the session manifest and the
//! interface cannot disagree about which build is running. The shape is
//! `v0.2.1-abc1234`: the released version, and the commit it was built from.
//!
//! The manifest files and package manifests keep the bare `0.2.1`, because
//! that is all a Cargo version, an npm version or an MSI product version will
//! accept. The combined form is for people.

/// The released version, as the package manifests carry it.
pub const NUMBER: &str = env!("CARGO_PKG_VERSION");

/// The short commit this was built from, or `unknown` outside a checkout.
pub const BUILD_REF: &str = env!("SSR_BUILD_REF");

/// The full version, as it is shown and as it is written into a manifest.
pub const FULL: &str = concat!("v", env!("CARGO_PKG_VERSION"), "-", env!("SSR_BUILD_REF"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_full_version_is_the_tag_and_the_commit() {
        assert!(FULL.starts_with("v"));
        assert!(FULL.contains(NUMBER));
        assert!(FULL.ends_with(BUILD_REF));
        // `v0.2.1-abc1234`: exactly one separator between the two parts.
        let after_v = &FULL[1..];
        assert_eq!(after_v, format!("{NUMBER}-{BUILD_REF}"));
    }

    #[test]
    fn the_build_ref_is_a_short_commit_or_an_honest_admission() {
        assert!(
            BUILD_REF == "unknown" || BUILD_REF.chars().all(|c| c.is_ascii_hexdigit()),
            "unexpected build ref {BUILD_REF:?}"
        );
    }
}

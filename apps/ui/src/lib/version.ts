// What this build calls itself.
//
// The same shape the engine reports and writes into a session manifest —
// `v0.2.1-abc1234` — so a screenshot of the About box and a `session.json`
// found later name the same build.
//
// The package manifests keep the bare number, because that is all npm, Cargo
// and an MSI product version accept. This is the form for people.

/** The released version, as package.json carries it. */
export const VERSION_NUMBER = __APP_VERSION__

/** The short commit this was built from, or `unknown` outside a checkout. */
export const BUILD_REF = __BUILD_REF__

/** The full version: the tag and the commit. */
export const VERSION = `v${__APP_VERSION__}-${__BUILD_REF__}`

// A GUI binary has no console, so a failure at startup leaves no trace at all.
// Building with `--features console` keeps the console attached, which is the
// only practical way to see why the window did not appear.
#![cfg_attr(
    all(not(debug_assertions), not(feature = "console")),
    windows_subsystem = "windows"
)]

fn main() {
    ssr_desktop_lib::run()
}

//! Opt-in tracing.
//!
//! Audio capture problems are hard to reproduce and often hardware or policy
//! specific, so the engine can narrate what it is doing on request. Set
//! `SSR_TRACE=1` to turn it on.

use std::sync::atomic::{AtomicU8, Ordering};

static STATE: AtomicU8 = AtomicU8::new(0);

/// Whether tracing is switched on, resolved once from the environment.
pub fn enabled() -> bool {
    match STATE.load(Ordering::Relaxed) {
        0 => {
            let on = std::env::var("SSR_TRACE")
                .map(|v| !v.is_empty() && v != "0" && !v.eq_ignore_ascii_case("false"))
                .unwrap_or(false);
            STATE.store(if on { 2 } else { 1 }, Ordering::Relaxed);
            on
        }
        2 => true,
        _ => false,
    }
}

/// Write one trace line to stderr, prefixed with the current thread name.
pub fn emit(args: std::fmt::Arguments<'_>) {
    let thread = std::thread::current();
    let name = thread.name().unwrap_or("callback").to_string();
    eprintln!("[ssr {name}] {args}");
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        if $crate::trace::enabled() {
            $crate::trace::emit(format_args!($($arg)*));
        }
    };
}

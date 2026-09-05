//! Runtime assertions for the properties types cannot carry.
//!
//! # Why these panic
//!
//! A contract states something the code already believes. If one fails, the
//! process holds a belief about infrastructure that reality does not share —
//! and the next thing it would do is act on that belief against a real machine.
//! Continuing is the dangerous option, not stopping.
//!
//! This is safe precisely because Nerpa never leaves a machine half-configured
//! on the way out: a panic unwinds, the state lock and the open transport run
//! their destructors, and the run is recorded as failed rather than as done.
//! That is also why the release profile must not set `panic = "abort"`.
//!
//! # Why they stay on in release builds
//!
//! `debug_assert!` is the usual answer, and it is the wrong one here. The runs
//! that matter happen unattended, from release binaries, against production —
//! exactly the builds where `debug_assert!` is compiled out. Turn them off with
//! `--no-default-features` if a benchmark ever shows they matter, not by habit.

/// Asserts something the caller must guarantee before the call.
///
/// A failure means the bug is in the caller.
#[macro_export]
macro_rules! requires {
    ($cond:expr) => {
        $crate::contract::check(cfg!(feature = "strict-contracts"), $cond, "precondition", stringify!($cond), None)
    };
    ($cond:expr, $($arg:tt)+) => {
        $crate::contract::check(
            cfg!(feature = "strict-contracts"),
            $cond,
            "precondition",
            stringify!($cond),
            Some(format_args!($($arg)+)),
        )
    };
}

/// Asserts something this function guarantees to its caller on the way out.
///
/// A failure means the bug is here.
#[macro_export]
macro_rules! ensures {
    ($cond:expr) => {
        $crate::contract::check(cfg!(feature = "strict-contracts"), $cond, "postcondition", stringify!($cond), None)
    };
    ($cond:expr, $($arg:tt)+) => {
        $crate::contract::check(
            cfg!(feature = "strict-contracts"),
            $cond,
            "postcondition",
            stringify!($cond),
            Some(format_args!($($arg)+)),
        )
    };
}

/// Asserts something that holds of a value at rest, independent of any call.
#[macro_export]
macro_rules! invariant {
    ($cond:expr) => {
        $crate::contract::check(cfg!(feature = "strict-contracts"), $cond, "invariant", stringify!($cond), None)
    };
    ($cond:expr, $($arg:tt)+) => {
        $crate::contract::check(
            cfg!(feature = "strict-contracts"),
            $cond,
            "invariant",
            stringify!($cond),
            Some(format_args!($($arg)+)),
        )
    };
}

/// The one place a contract failure is turned into a panic.
///
/// Kept as a function rather than inlined into each macro so that the message
/// has a single shape, and so that a breakpoint here catches every kind.
///
/// # Panics
///
/// Panics when `enabled` is true and `holds` is false. That is the entire
/// purpose: see the module documentation for why stopping is the safe direction.
#[doc(hidden)]
#[inline]
#[track_caller]
#[allow(
    clippy::panic,
    reason = "turning a broken belief into a panic is this function's job"
)]
pub fn check(
    enabled: bool,
    holds: bool,
    kind: &str,
    source: &str,
    detail: Option<std::fmt::Arguments<'_>>,
) {
    if !enabled || holds {
        return;
    }
    match detail {
        Some(detail) => panic!("{kind} violated: {source}: {detail}"),
        None => panic!("{kind} violated: {source}"),
    }
}

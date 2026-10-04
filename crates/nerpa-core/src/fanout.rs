//! Speaking to several machines at once, and to no more than a few.
//!
//! Decision `0031` chose threads over `async` and required the fan-out to be
//! **bounded**: two hundred machines must not mean two hundred of anything. The
//! bound is doing more work than that record says, and `W-014` records why —
//! one machine is one `ssh` child on this machine, with its own key exchange and
//! its own memory, so the ceiling is process-shaped and `async` would not have
//! saved a single one of them.
//!
//! # What it is worth, measured
//!
//! On a fleet of two it is worth nothing, and knowing why matters more than the
//! number. A bare `ssh` to the machine behind an identity-aware proxy takes
//! 5.07 s and to the one behind a bastion 3.11 s; a whole run against them takes
//! 5.06 s and 3.65 s. The conversation is half a second and the handshake is the
//! rest — and `Piped` spawns its `ssh` without waiting for that handshake, so
//! every machine is already shaking hands at once whether anything here is
//! threaded or not. Fanning the journeys out changed the wall clock by less than
//! the noise, measured both ways on the same fleet.
//!
//! It is kept because the case it is for is the one that has not arrived:
//! machines that *work* rather than wait. Installing a package takes seconds of
//! the machine's own time and nothing overlaps it, and two hundred of those in a
//! row is an afternoon.
//!
//! # Why a queue rather than a slice each
//!
//! Handing each thread a fixed share is one line shorter and wrong for the case
//! that matters: a machine that will not answer holds its share for the whole
//! connect timeout while the others sit idle. Taking the next piece of work when
//! free means a dead machine costs its own timeout and nobody else's.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// How many machines are spoken to at once.
///
/// Sixteen because it is comfortably under what a workstation's file
/// descriptors and an `ssh` agent tolerate, and comfortably over the number of
/// machines anybody has while reading the output. Ansible ships `forks = 5` and
/// everybody raises it; starting nearer the number people end up with saves the
/// same discovery.
pub const AT_ONCE: usize = 16;

/// Does `each` to every piece of work, a few at a time, and answers in order.
///
/// The order of the results is the order of the work, whatever order the
/// threads finished in — because a plan that reordered itself by which machine
/// answered first would be a plan that reads differently every run, and the
/// number over it would never settle.
pub fn every<T, R>(work: &[T], each: impl Fn(&T) -> R + Sync) -> Vec<R>
where
    T: Sync,
    R: Send,
{
    if work.len() < 2 {
        return work.iter().map(each).collect();
    }

    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<(usize, R)>> = Mutex::new(Vec::with_capacity(work.len()));
    let hands = AT_ONCE.min(work.len());

    std::thread::scope(|scope| {
        for _ in 0..hands {
            scope.spawn(|| {
                loop {
                    let at = next.fetch_add(1, Ordering::Relaxed);
                    let Some(piece) = work.get(at) else {
                        return;
                    };
                    let answer = each(piece);
                    // A poisoned lock here means another thread panicked while
                    // holding it, and this one has an answer nobody will read.
                    // Dropping it is what happens either way: the panic is on
                    // its way out of the scope.
                    if let Ok(mut done) = done.lock() {
                        done.push((at, answer));
                    }
                }
            });
        }
    });

    let mut answers = match done.into_inner() {
        Ok(answers) => answers,
        // The scope has joined every worker before this point, so nothing
        // panicked to poison the lock — and a poisoned one still holds every
        // answer collected before whatever went wrong.
        Err(poisoned) => poisoned.into_inner(),
    };
    answers.sort_by_key(|(at, _)| *at);
    answers.into_iter().map(|(_, answer)| answer).collect()
}

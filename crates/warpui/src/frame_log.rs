//! Slow-frame accounting for a build you run yourself.
//!
//! Upstream ships exactly one piece of frame-cost instrumentation,
//! `FeatureFlag::LogExpensiveFramesInSentry`, and this fork force-disables it
//! along with the rest of the telemetry flags. That call was right — it
//! reports to Sentry — but its consequence was not intended: the fork could
//! not answer "why does this feel slow" with a number, which only became
//! obvious the first time somebody said a drag felt laggy.
//!
//! This is the local replacement, in the same shape as the fork's other
//! answers to this problem (a local transcriber, a local agent transport, a
//! local OTLP export): it writes to the log file on this machine and has no
//! network path at all. It is off unless `WARP_FORK_FRAME_LOG` asks for it —
//! see `fork::slow_frame_threshold`, which owns the policy. This module owns
//! only the accounting.
//!
//! **It reports one line per second, not one per slow frame.** A line per
//! frame would be its own performance problem during exactly the stutter it
//! is trying to describe, and would change the thing being measured.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Sentinel for "no threshold set", so the per-frame check is a single relaxed
/// load and costs nothing in a build that never turns this on.
const DISABLED: u64 = u64::MAX;

/// How long slow frames accumulate before one summary line is emitted.
const SUMMARY_INTERVAL: Duration = Duration::from_secs(1);

static THRESHOLD_MICROS: AtomicU64 = AtomicU64::new(DISABLED);
static PENDING: Mutex<Option<Summary>> = Mutex::new(None);

/// Slow frames seen since the last line was emitted.
struct Summary {
    opened: Instant,
    count: u32,
    worst: Duration,
    total: Duration,
}

impl Summary {
    fn opened_at(now: Instant) -> Self {
        Self {
            opened: now,
            count: 0,
            worst: Duration::ZERO,
            total: Duration::ZERO,
        }
    }

    fn add(&mut self, elapsed: Duration) {
        self.count += 1;
        self.worst = self.worst.max(elapsed);
        self.total += elapsed;
    }

    fn mean(&self) -> Duration {
        self.total.checked_div(self.count).unwrap_or(Duration::ZERO)
    }
}

/// Sets the threshold above which a frame is worth reporting, or `None` to
/// switch reporting off. Called once during early startup from the app's fork
/// policy; safe to call again.
pub fn set_threshold(threshold: Option<Duration>) {
    let micros = match threshold {
        // A zero threshold would report every frame, which is the failure mode
        // this module exists to avoid. Treat it as "off" rather than as "all".
        Some(threshold) if !threshold.is_zero() => {
            u64::try_from(threshold.as_micros()).unwrap_or(DISABLED - 1)
        }
        _ => DISABLED,
    };
    THRESHOLD_MICROS.store(micros, Ordering::Relaxed);
}

/// Whether frames are being timed. Checked before taking an [`Instant`], so a
/// disabled build pays one atomic load per frame and nothing else.
pub fn is_enabled() -> bool {
    THRESHOLD_MICROS.load(Ordering::Relaxed) != DISABLED
}

/// Records one frame's render duration. Frames under the threshold cost a
/// single load and return; the lock is only taken for frames already slow
/// enough that a mutex is not what is wrong with them.
pub fn record(elapsed: Duration) {
    let threshold = THRESHOLD_MICROS.load(Ordering::Relaxed);
    if threshold == DISABLED {
        return;
    }
    let elapsed_micros = u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX);
    if elapsed_micros < threshold {
        return;
    }

    let now = Instant::now();
    let Ok(mut pending) = PENDING.lock() else {
        return;
    };
    let summary = pending.get_or_insert_with(|| Summary::opened_at(now));
    summary.add(elapsed);

    if now.duration_since(summary.opened) < SUMMARY_INTERVAL {
        return;
    }
    let elapsed_window = now.duration_since(summary.opened);
    log::warn!(
        "Slow frames: {} in {:.1}s (worst {:.1}ms, mean {:.1}ms, threshold {:.1}ms)",
        summary.count,
        elapsed_window.as_secs_f32(),
        summary.worst.as_secs_f32() * 1000.,
        summary.mean().as_secs_f32() * 1000.,
        threshold as f32 / 1000.,
    );
    *pending = None;
}

/// Drops any accumulated summary without emitting it. For tests, which share
/// this process-wide state.
#[cfg(test)]
pub(crate) fn reset() {
    THRESHOLD_MICROS.store(DISABLED, Ordering::Relaxed);
    if let Ok(mut pending) = PENDING.lock() {
        *pending = None;
    }
}

/// The number of slow frames currently accumulated, for tests.
#[cfg(test)]
pub(crate) fn pending_count() -> u32 {
    PENDING
        .lock()
        .ok()
        .and_then(|pending| pending.as_ref().map(|summary| summary.count))
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "frame_log_tests.rs"]
mod tests;

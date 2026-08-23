use std::sync::Mutex;
use std::time::Duration;

use super::{is_enabled, pending_count, record, reset, set_threshold};

/// The module's state is process-wide, so the tests take turns.
static SERIAL: Mutex<()> = Mutex::new(());

#[test]
fn frames_are_not_timed_until_a_threshold_is_set() {
    let _guard = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    reset();

    assert!(!is_enabled());
    // A frame that would be catastrophically slow is still not recorded,
    // because nothing asked for it.
    record(Duration::from_secs(10));
    assert_eq!(pending_count(), 0);
}

#[test]
fn only_frames_over_the_threshold_are_recorded() {
    let _guard = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    reset();
    set_threshold(Some(Duration::from_millis(33)));

    assert!(is_enabled());
    record(Duration::from_millis(16));
    record(Duration::from_millis(32));
    assert_eq!(pending_count(), 0, "frames under the threshold are ignored");

    record(Duration::from_millis(33));
    record(Duration::from_millis(120));
    assert_eq!(pending_count(), 2);

    reset();
}

#[test]
fn a_zero_threshold_means_off_rather_than_report_everything() {
    let _guard = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    reset();

    // Reporting every frame would be its own performance problem, so zero is
    // read as "off" — the same answer as `None`.
    set_threshold(Some(Duration::ZERO));
    assert!(!is_enabled());
    record(Duration::from_millis(500));
    assert_eq!(pending_count(), 0);

    set_threshold(None);
    assert!(!is_enabled());

    reset();
}

#[test]
fn slow_frames_accumulate_into_one_summary_rather_than_a_line_each() {
    let _guard = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    reset();
    set_threshold(Some(Duration::from_millis(10)));

    // Ten slow frames inside one summary interval stay pending as a single
    // accumulating record; nothing is emitted per frame.
    for _ in 0..10 {
        record(Duration::from_millis(50));
    }
    assert_eq!(pending_count(), 10);

    reset();
}

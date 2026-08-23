//! Unit tests for [`CrossWindowTabDrag`] placeholder-collapse policy.
//!
//! These focus on [`CrossWindowTabDrag::collapsed_source_placeholder_index`],
//! which decides whether the source window's horizontal tab bar collapses the
//! detached-placeholder slot to zero width. The regression these guard against
//! is the horizontal "fuzzy shake": collapsing the placeholder while the cursor
//! is reordering it back in the source window removed the visible drop zone and
//! made the slot oscillate every frame.

use warpui::WindowId;
use warpui::geometry::vector::{Vector2F, vec2f};

use super::CrossWindowTabDrag;

const SOURCE_TAB_INDEX: usize = 2;

fn begin_multi_tab_drag(
    drag: &mut CrossWindowTabDrag,
    source_window_id: WindowId,
    preview_window_id: WindowId,
) {
    drag.begin_multi_tab_drag(
        source_window_id,
        SOURCE_TAB_INDEX,
        Vector2F::zero(),
        vec2f(800.0, 600.0),
        Vector2F::zero(),
        preview_window_id,
        false,
        vec2f(120.0, 34.0),
    );
}

#[test]
fn no_active_drag_keeps_all_slots_full_width() {
    let drag = CrossWindowTabDrag::new();
    assert_eq!(
        drag.collapsed_source_placeholder_index(WindowId::from_usize(1)),
        None
    );
}

#[test]
fn multi_tab_drag_collapses_only_the_source_window_placeholder() {
    let source = WindowId::from_usize(1);
    let preview = WindowId::from_usize(2);
    let other = WindowId::from_usize(3);

    let mut drag = CrossWindowTabDrag::new();
    begin_multi_tab_drag(&mut drag, source, preview);

    // The source window collapses its detached placeholder while the tab is
    // floating in the preview window.
    assert_eq!(
        drag.collapsed_source_placeholder_index(source),
        Some(SOURCE_TAB_INDEX)
    );
    // The preview and unrelated windows never collapse a slot.
    assert_eq!(drag.collapsed_source_placeholder_index(preview), None);
    assert_eq!(drag.collapsed_source_placeholder_index(other), None);
}

#[test]
fn source_reorder_keeps_placeholder_full_width() {
    let source = WindowId::from_usize(1);
    let preview = WindowId::from_usize(2);

    let mut drag = CrossWindowTabDrag::new();
    begin_multi_tab_drag(&mut drag, source, preview);

    // Cursor returns to the source's own tab bar: the placeholder is reordered
    // in place like an in-window drag and must stay full width. Collapsing it
    // here is what produced the horizontal "fuzzy shake".
    drag.set_reordering_in_source_for_test(true);
    assert_eq!(drag.collapsed_source_placeholder_index(source), None);

    // Leaving the source again restores the zero-width collapse.
    drag.set_reordering_in_source_for_test(false);
    assert_eq!(
        drag.collapsed_source_placeholder_index(source),
        Some(SOURCE_TAB_INDEX)
    );
}

#[test]
fn single_tab_drag_never_collapses_a_slot() {
    let source = WindowId::from_usize(1);

    let mut drag = CrossWindowTabDrag::new();
    // A single-tab window is its own floating preview; there is no separate
    // placeholder to collapse.
    drag.begin_single_tab_drag(
        source,
        Vector2F::zero(),
        vec2f(800.0, 600.0),
        Vector2F::zero(),
        false,
        vec2f(120.0, 34.0),
    );

    assert_eq!(drag.collapsed_source_placeholder_index(source), None);
}

/// T9.3: a tab in flight between windows must stop advertising itself as a
/// pane-drop candidate.
///
/// The bug this pins was not in this file — it was in the fork's tab-to-pane
/// drop source. During a cross-window drag the source window follows the
/// cursor, so its *own* pane sits under the drag rect and was the smallest
/// intersecting drop target. The release therefore dispatched `DropTabOnPane`
/// and never `DropTab`, `CrossWindowTabDrag::on_drop` was never called, and the
/// ghost drawn in the target window stayed there.
///
/// The two states are mutually exclusive by design, so assert it directly:
/// the moment a cross-window drag begins, the pane drop target is refused.
#[test]
fn a_tab_in_flight_between_windows_refuses_pane_drop_targets() {
    let source = WindowId::from_usize(1);
    let preview = WindowId::from_usize(2);

    let mut drag = CrossWindowTabDrag::new();
    assert!(!drag.is_active());
    assert!(
        crate::fork::tab_pane_drop_target_accepted_while(drag.is_active()),
        "with no drag in flight a tab must still be able to land on a pane"
    );

    // The single-tab case is the one the user hit: drag the sole tab of a
    // torn-out window back towards another window's strip.
    drag.begin_single_tab_drag(
        source,
        Vector2F::zero(),
        vec2f(800.0, 600.0),
        Vector2F::zero(),
        false,
        vec2f(120.0, 34.0),
    );
    assert!(drag.is_active());
    assert!(
        !crate::fork::tab_pane_drop_target_accepted_while(drag.is_active()),
        "a tab already in flight between windows must not also be a pane drop"
    );

    // And the multi-tab case, which is how a tab leaves a populated strip.
    let mut drag = CrossWindowTabDrag::new();
    begin_multi_tab_drag(&mut drag, source, preview);
    assert!(
        !crate::fork::tab_pane_drop_target_accepted_while(drag.is_active()),
        "the multi-tab path strands the same state machine"
    );
}

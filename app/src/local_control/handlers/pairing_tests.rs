use super::*;

/// A QR needs its quiet zone to be *quiet*. This is the assertion that caught a
/// real bug while the renderer was being written: `QrMatrix::is_dark` indexes
/// `y * width + x` into a flat `Vec`, so an `x` past the right edge does not
/// read out of bounds and return false — it wraps into the next row. Without an
/// explicit width check the right-hand margin was a strip of the following row's
/// modules, which no scanner would have read and no test of the *centre* would
/// have noticed.
#[test]
fn the_quiet_zone_is_actually_quiet() {
    let rendered = render_qr("http://192.168.1.5:41234/v1/pair#abc").expect("a QR");
    let rows: Vec<&str> = rendered.lines().collect();

    // Half blocks put two module rows in one text row, so two text rows cover
    // the four-module quiet zone at top and bottom.
    let margin = QUIET_ZONE_MODULES / 2;
    for row in rows
        .iter()
        .take(margin)
        .chain(rows.iter().rev().take(margin))
    {
        assert!(
            row.chars().all(|cell| cell == '█'),
            "a margin row must be entirely light, got {row:?}"
        );
    }
    for row in &rows {
        let cells: Vec<char> = row.chars().collect();
        assert!(
            cells[..QUIET_ZONE_MODULES].iter().all(|cell| *cell == '█'),
            "the left margin must be light, got {row:?}"
        );
        assert!(
            cells[cells.len() - QUIET_ZONE_MODULES..]
                .iter()
                .all(|cell| *cell == '█'),
            "the right margin must be light — this is the one that wrapped"
        );
    }
}

/// The rendering is square-ish and complete: every row is the same width, and
/// there are half as many rows as columns because each holds two module rows.
#[test]
fn the_code_is_rendered_two_module_rows_at_a_time() {
    let rendered = render_qr("http://192.168.1.5:41234/v1/pair#abc").expect("a QR");
    let rows: Vec<&str> = rendered.lines().collect();
    let width = rows[0].chars().count();

    for row in &rows {
        assert_eq!(row.chars().count(), width, "ragged row {row:?}");
    }
    assert_eq!(rows.len(), width.div_ceil(2));
}

/// Light modules are painted rather than skipped. A terminal's background is
/// usually dark, so "leave the light modules blank" produces a code that is dark
/// where it must be light — unscannable, and unscannable in a way that looks
/// fine in a diff.
#[test]
fn light_modules_are_drawn_and_not_left_to_the_background() {
    let rendered = render_qr("http://192.168.1.5:41234/v1/pair#abc").expect("a QR");

    assert!(rendered.contains('█'), "light modules must be painted");
    assert!(
        rendered.contains(' '),
        "dark modules must be left unpainted"
    );
}

/// With no wide listener there is nothing a code could be spent against, so
/// minting one would put a secret on screen for nothing. The refusal names the
/// variable because this error is the whole discovery path for the feature.
#[test]
fn there_is_nothing_to_pair_with_until_a_wide_listener_exists() {
    let error = control_pair(None).expect_err("refused");

    assert_eq!(error.code, ErrorCode::LocalControlDisabled);
    assert!(error.message.contains("WARP_FORK_CONTROL_BIND"));
}

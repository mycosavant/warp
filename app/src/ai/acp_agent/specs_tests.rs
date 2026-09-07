use super::*;

/// The compiled-in file parses and holds the four Claude Code families, each
/// priced and ranked, with the date its prices were read. A typo in the
/// default table would otherwise be a `?` on every bar and no error anywhere.
#[test]
fn the_default_table_parses_and_prices_every_family() {
    let table = Table::defaults();
    assert_eq!(table.as_of.as_deref(), Some("2026-09-07"));
    assert!(
        table.source.is_some(),
        "the table names where the prices came from"
    );
    for id in ["fable", "opus", "sonnet", "haiku"] {
        let row = table
            .row(id, None)
            .unwrap_or_else(|| panic!("a row for {id}"));
        assert!(
            row.input.is_some() && row.output.is_some(),
            "{id} is priced"
        );
        assert!(
            row.intelligence.is_some() && row.speed.is_some(),
            "{id} is ranked"
        );
        assert!(row.class.is_some(), "{id} has its one word");
    }
}

/// The lookup keys the module docs promise: the id, the id without a
/// bracketed suffix, and for a description with no separator its first word.
#[test]
fn a_row_is_found_by_id_stem_or_the_first_word_of_a_plain_description() {
    let table = Table::defaults();
    assert_eq!(
        table.row("opus[1m]", None).map(|row| row.id.as_str()),
        Some("opus")
    );
    assert_eq!(
        table
            .row("default", Some("Opus (1M context)"))
            .map(|row| row.id.as_str()),
        Some("opus"),
        "Claude Code's default row is described as the model it resolves to"
    );
    assert_eq!(
        table
            .row("fable", Some("Fable 5.1 · Most capable"))
            .map(|row| row.id.as_str()),
        Some("fable"),
        "the id wins when it is known, whatever the description says"
    );
    assert!(
        table
            .row(
                "openrouter/anthropic/claude-sonnet-4",
                Some("Sonnet 4 · Balanced")
            )
            .is_none(),
        "a description with a separator is a tagline, not a lookup key"
    );
    assert!(table.row("gpt-5", None).is_none());
}

/// Cost is relative to the dearest model offered, dearest at full width, so
/// the same row draws differently depending on what else the agent offers.
#[test]
fn cost_is_output_price_against_the_dearest_model_offered() {
    let table = Table::defaults();
    let all = [
        ("fable", None),
        ("opus", None),
        ("sonnet", None),
        ("haiku", None),
    ];
    let dearest = table.dearest_output(all);
    assert_eq!(dearest, Some(50.0));
    let sonnet = table.spec("sonnet", None, dearest).expect("known");
    assert!(
        (sonnet.cost - 0.2).abs() < 1e-6,
        "10 of 50: {}",
        sonnet.cost
    );
    assert!((sonnet.quality - 0.7).abs() < 1e-6);
    assert!((sonnet.speed - 0.8).abs() < 1e-6);

    let without_fable = table.dearest_output([("sonnet", None), ("haiku", None)]);
    assert_eq!(without_fable, Some(10.0));
    let sonnet = table.spec("sonnet", None, without_fable).expect("known");
    assert!(
        (sonnet.cost - 1.0).abs() < 1e-6,
        "dearest on offer is full width"
    );
}

/// With no row, the vendor's own ordering stands in for the ranking and the
/// cost bar is unknown; with neither a row nor a tagline there is no spec.
#[test]
fn the_tagline_tier_stands_in_for_a_missing_row_and_cost_stays_unknown() {
    let table = Table::defaults();
    let spec = table
        .spec(
            "mystery",
            Some("Mystery 9 · Fastest for quick answers"),
            Some(50.0),
        )
        .expect("a tier is a spec");
    assert_eq!(bar(spec.cost), None, "no price, no cost bar");
    assert_eq!(bar(spec.quality), Some(0.25));
    assert_eq!(bar(spec.speed), Some(1.0));
    assert_eq!(
        table.spec("mystery", Some("Mystery 9 · Balanced"), Some(50.0)),
        None,
        "a sentence that names no tier is no ranking"
    );
    assert_eq!(table.spec("mystery", None, None), None);
}

/// A person's file lays over the defaults by id: a replaced row, an added
/// row, and the file's own date, with the untouched rows still there.
#[test]
fn a_users_file_replaces_rows_by_id_and_adds_the_rest() {
    let mut table = Table::defaults();
    let file: File = toml::from_str(
        r#"
as_of = "2026-10-01"

[[model]]
id = "sonnet"
class = "Workhorse"
input = 3.0
output = 15.0

[[model]]
id = "gpt-5"
class = "Other"
output = 12.0
"#,
    )
    .expect("parses");
    table.overlay(file);
    assert_eq!(table.as_of.as_deref(), Some("2026-10-01"));
    assert!(
        table.source.is_some(),
        "a file that states no source keeps the default's"
    );
    let sonnet = table.row("sonnet", None).expect("still there");
    assert_eq!(sonnet.class.as_deref(), Some("Workhorse"));
    assert_eq!(sonnet.output, Some(15.0));
    assert_eq!(
        sonnet.intelligence, None,
        "a replaced row is the file's row whole, not a merge of fields"
    );
    assert_eq!(
        table.row("gpt-5", None).and_then(|row| row.output),
        Some(12.0)
    );
    assert!(table.row("haiku", None).is_some(), "untouched rows stay");
}

/// The card's price line, and the one word for the menu.
#[test]
fn the_price_line_and_the_class_read_from_the_row() {
    let table = Table::defaults();
    assert_eq!(
        table.price_line("fable", None).as_deref(),
        Some("$10 in · $50 out per million tokens, list price as of 2026-09-07")
    );
    assert_eq!(table.class("opus[1m]", None).as_deref(), Some("Flagship"));
    assert_eq!(table.price_line("gpt-5", None), None);
    assert_eq!(dollars(2.5), "2.50");
    assert_eq!(dollars(10.0), "10");
}

/// The tagline is what follows the separator, or the whole description when
/// there is none; blank is nothing.
#[test]
fn the_tagline_is_the_sentence_after_the_separator_or_the_whole_description() {
    assert_eq!(
        tagline("Fable 5.1 · Most capable for your hardest and longest-running tasks"),
        Some("Most capable for your hardest and longest-running tasks")
    );
    assert_eq!(tagline("Opus (1M context)"), Some("Opus (1M context)"));
    assert_eq!(tagline("Fable 5.1 · "), None);
    assert_eq!(tagline("  "), None);
}

/// A malformed user file is the defaults, logged, not a lost picker.
#[test]
fn a_malformed_file_is_ignored_in_favour_of_the_defaults() {
    assert!(Table::parse("this is = not [ toml").is_err());
    let parsed: File = toml::from_str("").expect("an empty file is an empty overlay");
    let mut table = Table::defaults();
    let before = table.clone();
    table.overlay(parsed);
    assert_eq!(table, before);
}

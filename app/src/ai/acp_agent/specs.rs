//! What the Model Specs card can say about an agent's model (T21.2, 2026-09-07).
//!
//! Upstream's card draws three bars from `LLMSpec { cost, quality, speed }`,
//! floats its server sends beside each model, under a header that calls them
//! Warp's benchmarks. The Agent Client Protocol sends an id, a name and a
//! sentence per model, and no number, so for an agent's list every bar is a
//! value this fork supplies. This module is where they come from, and the
//! card's header for such a list ([`AGENT_MODEL_SPECS_DESCRIPTION`]) says so.
//!
//! # The table
//!
//! [`Table`] is `specs.default.toml`, compiled in, with
//! `<state dir>/acp-model-specs.toml` read on top of it when present: a row
//! there replaces the default row with the same id, a new id is added, and the
//! file's `as_of` and `source` replace the defaults. The person edits a file
//! and restarts nothing; the table is reloaded on every `session/new`.
//!
//! The choice of a table over a fetch of a public price list is recorded in
//! `.fork/tickets/T21-the-agents-models.md`: the mapping from the agent's ids
//! to a price row is a table in either design, and the fetch would be the
//! fork's first outbound request to a party that is not the person's own
//! agent, through a deny-list that would let it pass. If the table goes stale
//! often enough to matter, the fetch is the next step, behind a switch.
//!
//! # What the numbers are
//!
//! - **Cost** is the vendor's list price for output tokens, drawn relative to
//!   the dearest model the agent currently offers, dearest at full width. On
//!   a subscription no turn costs dollars, but the ratio is roughly how fast a
//!   turn spends the rate-limit window, and with a single vendor's list price
//!   and capability move together, which is why the maintainer's sketch of
//!   the bars "more or less stands" for `claude-agent-acp`.
//! - **Intelligence** and **speed** are a ranking kept in this fork, 0..1,
//!   written in the table as an opinion so it can be argued with. When the
//!   table has no row, the vendor's own ordering stands in: Claude Code's
//!   four taglines name a tier each ([`tier`]), and a tier is a coarse pair.
//!   When neither exists the bar draws `?`, which is what the card did before
//!   this module.
//!
//! # Unknown is `-1.0`
//!
//! `LLMSpec` is three plain `f32`s and upstream's card treats them as all
//! known or all absent. A row can be half known (a price with no ranking, or
//! a tagline with no price), so a field this module cannot fill is written
//! as `-1.0` and read back through [`bar`], which turns it into the `?` the
//! card already draws for an absent spec. The settings page draws `LLMSpec`
//! straight into a clamped bar, so there an unknown is an empty bar; that
//! page's header is swapped for the same honest one and nothing more.

use std::sync::{Arc, RwLock};

use serde::Deserialize;

use crate::ai::llms::LLMSpec;

/// The compiled-in table, beside this file.
const DEFAULT_TABLE: &str = include_str!("specs.default.toml");

/// The file a person edits, under the fork's state directory.
pub(crate) const USER_TABLE_FILE: &str = "acp-model-specs.toml";

/// The card's header for an agent's list, replacing upstream's *Warp's
/// benchmarks…*, which describes floats Warp's server sends for Warp's own
/// agent and nothing about a list that came from `session/new`.
///
/// Two lines at the card's width, measured: the first cut was three, and
/// with the two lines under it the Cost row fell off the bottom of a menu
/// whose height does not follow its details pane.
pub(crate) const AGENT_MODEL_SPECS_DESCRIPTION: &str = "Cost: output list price against the dearest model offered. Intelligence and speed: this fork's ranking, not a benchmark.";

/// Unknown, for a field of `LLMSpec` this module cannot fill. See the module
/// docs.
pub(crate) const UNKNOWN: f32 = -1.0;

/// A bar's value as the card should draw it: `None` for [`UNKNOWN`], which
/// the card draws as `?`.
pub(crate) fn bar(value: f32) -> Option<f32> {
    (value >= 0.0).then_some(value)
}

/// One model the table knows. Prices are USD per million tokens, list price;
/// ranks are 0..1.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct Row {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) class: Option<String>,
    #[serde(default)]
    pub(crate) input: Option<f32>,
    #[serde(default)]
    pub(crate) output: Option<f32>,
    #[serde(default)]
    pub(crate) intelligence: Option<f32>,
    #[serde(default)]
    pub(crate) speed: Option<f32>,
}

/// The table as parsed from one file.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
struct File {
    #[serde(default)]
    as_of: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    model: Vec<Row>,
}

/// The defaults with the person's file laid over them.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Table {
    /// The date the prices were read off the vendor's page, as written in
    /// the file; the card shows it beside a price.
    pub(crate) as_of: Option<String>,
    pub(crate) source: Option<String>,
    rows: Vec<Row>,
}

static CURRENT: RwLock<Option<Arc<Table>>> = RwLock::new(None);

impl Table {
    /// The compiled-in table alone.
    pub(crate) fn defaults() -> Self {
        Self::parse(DEFAULT_TABLE)
            .map(Self::from_file)
            .unwrap_or_default()
    }

    /// The defaults with `<state dir>/acp-model-specs.toml` on top, and made
    /// current for the card. A missing file is the defaults; an unreadable
    /// or malformed one is logged and is also the defaults, because a typo
    /// in a price should cost a `?`, not the picker.
    pub(crate) fn load() -> Arc<Self> {
        let path = crate::fork::state_dir().join(USER_TABLE_FILE);
        let mut table = Self::defaults();
        match std::fs::read_to_string(&path) {
            Ok(text) => match Self::parse(&text) {
                Ok(file) => table.overlay(file),
                Err(error) => log::warn!(
                    "fork: {} is not a specs table and is ignored: {error}",
                    path.display()
                ),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => log::warn!("fork: {} unreadable, ignored: {error}", path.display()),
        }
        let table = Arc::new(table);
        if let Ok(mut current) = CURRENT.write() {
            *current = Some(Arc::clone(&table));
        }
        table
    }

    /// The table the last [`load`] produced, loading if none has run: the
    /// card reads through this, per frame, and must not touch a file.
    pub(crate) fn current() -> Arc<Self> {
        if let Some(table) = CURRENT.read().ok().and_then(|current| current.clone()) {
            return table;
        }
        Self::load()
    }

    fn parse(text: &str) -> Result<File, toml::de::Error> {
        toml::from_str(text)
    }

    fn from_file(file: File) -> Self {
        Self {
            as_of: file.as_of,
            source: file.source,
            rows: file.model,
        }
    }

    /// Lay one file over this table: same id replaces, new id appends, and
    /// the file's date and source win when it states them.
    fn overlay(&mut self, file: File) {
        for row in file.model {
            match self
                .rows
                .iter_mut()
                .find(|known| known.id.eq_ignore_ascii_case(&row.id))
            {
                Some(known) => *known = row,
                None => self.rows.push(row),
            }
        }
        if file.as_of.is_some() {
            self.as_of = file.as_of;
        }
        if file.source.is_some() {
            self.source = file.source;
        }
    }

    /// The row for one of the agent's models, by the keys the module docs
    /// list: the id, the id without a bracketed suffix, and for a description
    /// with no ` · ` its first word.
    pub(crate) fn row(&self, id: &str, description: Option<&str>) -> Option<&Row> {
        let find = |key: &str| {
            self.rows
                .iter()
                .find(|row| row.id.eq_ignore_ascii_case(key))
        };
        if let Some(row) = find(id) {
            return Some(row);
        }
        if let Some((stem, _)) = id.split_once('[')
            && let Some(row) = find(stem)
        {
            return Some(row);
        }
        let description = description?;
        if description.contains(SEPARATOR) {
            return None;
        }
        let first = description
            .split(|c: char| !c.is_alphanumeric())
            .find(|word| !word.is_empty())?;
        find(first)
    }

    /// The three bars for one of the agent's models, with [`UNKNOWN`] where
    /// the table and the tagline are both silent, or `None` when all three
    /// are. `dearest` is the highest output price among the models the agent
    /// offers, from [`dearest_output`].
    pub(crate) fn spec(
        &self,
        id: &str,
        description: Option<&str>,
        dearest: Option<f32>,
    ) -> Option<LLMSpec> {
        let row = self.row(id, description);
        let cost = match (row.and_then(|row| row.output), dearest) {
            (Some(output), Some(dearest)) if dearest > 0.0 => (output / dearest).clamp(0.0, 1.0),
            _ => UNKNOWN,
        };
        let fallback = description.and_then(tagline).and_then(tier);
        let quality = row
            .and_then(|row| row.intelligence)
            .or(fallback.map(|(quality, _)| quality))
            .unwrap_or(UNKNOWN);
        let speed = row
            .and_then(|row| row.speed)
            .or(fallback.map(|(_, speed)| speed))
            .unwrap_or(UNKNOWN);
        (cost >= 0.0 || quality >= 0.0 || speed >= 0.0).then_some(LLMSpec {
            cost,
            quality,
            speed,
        })
    }

    /// The highest output price among the given models, which is what every
    /// cost bar is drawn against. `None` when no offered model has a price.
    pub(crate) fn dearest_output<'a>(
        &self,
        offered: impl IntoIterator<Item = (&'a str, Option<&'a str>)>,
    ) -> Option<f32> {
        offered
            .into_iter()
            .filter_map(|(id, description)| self.row(id, description)?.output)
            .fold(None, |dearest: Option<f32>, output| {
                Some(dearest.map_or(output, |dearest| dearest.max(output)))
            })
    }

    /// The one word the menu shows beside the name, when the table has one.
    pub(crate) fn class(&self, id: &str, description: Option<&str>) -> Option<String> {
        self.row(id, description)?.class.clone()
    }

    /// The line under the card's header that names the price, when the table
    /// has one: `$10 in · $50 out per million tokens, list price as of
    /// 2026-09-07`.
    pub(crate) fn price_line(&self, id: &str, description: Option<&str>) -> Option<String> {
        let row = self.row(id, description)?;
        let (input, output) = (row.input?, row.output?);
        let as_of = self
            .as_of
            .as_deref()
            .map(|date| format!(", list price as of {date}"))
            .unwrap_or_default();
        Some(format!(
            "${} in · ${} out per million tokens{as_of}",
            dollars(input),
            dollars(output)
        ))
    }
}

/// `$10`, `$2.50`: whole dollars without a fraction, otherwise two places.
fn dollars(value: f32) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

/// What `claude-agent-acp` puts between the version and the tagline.
const SEPARATOR: &str = " · ";

/// The sentence for the card: what follows ` · ` in the agent's description,
/// or the whole description when it has no separator (Claude Code's
/// `default` row is described as the model it resolves to).
pub(crate) fn tagline(description: &str) -> Option<&str> {
    let text = match description.split_once(SEPARATOR) {
        Some((_, tagline)) => tagline,
        None => description,
    }
    .trim();
    (!text.is_empty()).then_some(text)
}

/// The vendor's own ordering, as `(intelligence, speed)`, from the tier a
/// Claude Code tagline names. Coarse on purpose: four strings, four pairs,
/// and any other sentence is no tier at all.
pub(crate) fn tier(tagline: &str) -> Option<(f32, f32)> {
    let tagline = tagline.to_ascii_lowercase();
    if tagline.starts_with("most capable") {
        Some((1.0, 0.25))
    } else if tagline.starts_with("best for everyday") {
        Some((0.75, 0.5))
    } else if tagline.starts_with("efficient") {
        Some((0.5, 0.75))
    } else if tagline.starts_with("fastest") {
        Some((0.25, 1.0))
    } else {
        None
    }
}

#[cfg(test)]
#[path = "specs_tests.rs"]
mod tests;

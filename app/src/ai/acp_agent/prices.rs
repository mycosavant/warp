//! Prices for the Model Specs card, fetched instead of hand-written (T21.4,
//! 2026-09-09).
//!
//! [`super::specs`] is the mapping: an agent's model id to a row carrying a
//! class, a ranking and, until now, two numbers typed in from a vendor's
//! pricing page. Four rows were enough while the only agent offered five
//! models from one vendor. Measured 2026-09-07, `opencode` over OpenRouter
//! hands the picker **365** rows, and no table anyone maintains by hand
//! follows that.
//!
//! So this module fills the numbers, and **only the numbers**. The mapping
//! stays where it was, for the reason T21 recorded when it chose the table:
//! the fetch does not know that `opus` means `anthropic/claude-opus-5` this
//! month, and next year it will mean something else while the catalogue keeps
//! answering correctly for both slugs. A row in `specs.default.toml` names its
//! `slug`; an agent whose ids *are* slugs (any OpenRouter-backed agent) needs
//! no row at all, and that is the case this module exists for.
//!
//! # Off unless asked, by name
//!
//! [`crate::fork::model_prices_fetch`] reads `WARP_FORK_MODEL_PRICES=fetch`.
//! Nothing here builds a request without it — [`Plan::of`] is the only thing
//! that decides, it is a pure function, and `prices_tests.rs` pins that the
//! module constructs exactly one HTTP client and that it sits behind that
//! decision.
//!
//! **The deny-list is not the layer that decides this, and saying so is the
//! point.** `egress_policy` names telemetry vendors and Warp's own hosts;
//! `openrouter.ai` is on neither list, so it would pass without ever being
//! considered. A deny-list protects retroactively against hosts somebody
//! thought of. The variable is the consent.
//!
//! # What leaves the machine
//!
//! One `GET` with no query, no key and no body, to a public catalogue. It goes
//! through [`http_client::Client::get_without_warp_headers`] rather than
//! `get`, because on every non-wasm target `get` attaches the client id, the
//! app version and four fields describing the operating system down to the
//! kernel version — and in this fork the app version is `v0.fork.<sha>`, which
//! names the commit this binary was built from. None of that is needed to read
//! a price list. That call is still an ordinary `Client` request, so the
//! egress policy sees it; it is not a way out.
//!
//! At most one request per launch, cached, and the cache's age in days is
//! printed on the card beside the price. A number with no date is the stale
//! -doc defect drawn as a picture, which is the thing the card exists to
//! avoid.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The catalogue, and the only host this module ever names.
pub(crate) const SOURCE: &str = "https://openrouter.ai/api/v1/models";

/// The file under `fork::state_dir()`.
pub(crate) const CACHE_FILE: &str = "acp-model-prices.json";

/// How old a cache may be before the next launch refetches it.
///
/// Thirty days is not a freshness requirement — list prices move rarely, and
/// when they move the card shows the age so a reader can weigh it. It is a
/// ceiling on how wrong a number can quietly be.
const MAX_AGE_DAYS: i64 = 30;

/// A request that hangs must not hold up the first turn of the launch. Ten
/// seconds is long enough for a 700 KB body on a domestic link and short
/// enough that a person notices it once rather than blames the agent.
const TIMEOUT: Duration = Duration::from_secs(10);

/// One model's list price, USD per million tokens.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub(crate) struct Price {
    pub(crate) input: f32,
    pub(crate) output: f32,
}

/// What is on disk, and what the card reads.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub(crate) struct Catalogue {
    /// When the fetch that produced this ran. `None` for the empty catalogue
    /// every launch starts with.
    pub(crate) fetched: Option<DateTime<Utc>>,
    pub(crate) source: Option<String>,
    #[serde(default)]
    prices: BTreeMap<String, Price>,
}

static CURRENT: RwLock<Option<Arc<Catalogue>>> = RwLock::new(None);

impl Catalogue {
    /// The catalogue as it now stands: the cache file on the first call, then
    /// whatever [`refresh_once`] last installed. The card reads through this
    /// per frame and must not touch a file.
    pub(crate) fn current() -> Arc<Self> {
        if let Some(catalogue) = CURRENT.read().ok().and_then(|current| current.clone()) {
            return catalogue;
        }
        let catalogue = Arc::new(Self::from_cache());
        Self::install(&catalogue);
        catalogue
    }

    /// The cache file, or an empty catalogue. A malformed file is logged and
    /// is also empty, for `specs::Table::load`'s reason: a bad cache should
    /// cost a `?`, not the picker.
    fn from_cache() -> Self {
        let path = crate::fork::state_dir().join(CACHE_FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(catalogue) => catalogue,
                Err(error) => {
                    log::warn!(
                        "fork: {} is not a price catalogue and is ignored: {error}",
                        path.display()
                    );
                    Self::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => {
                log::warn!("fork: {} unreadable, ignored: {error}", path.display());
                Self::default()
            }
        }
    }

    fn install(catalogue: &Arc<Self>) {
        if let Ok(mut current) = CURRENT.write() {
            *current = Some(Arc::clone(catalogue));
        }
    }

    /// The price for one catalogue slug, exactly as written. Exact match on
    /// purpose: OpenRouter lists `anthropic/claude-opus-5` beside
    /// `anthropic/claude-opus-5:batch` at half the price, and a prefix rule
    /// would quietly draw the cheaper bar.
    pub(crate) fn price(&self, slug: &str) -> Option<Price> {
        self.prices.get(slug).copied()
    }

    /// Whole days since the fetch, for the line under the card's header.
    pub(crate) fn age_days(&self, now: DateTime<Utc>) -> Option<i64> {
        Some((now - self.fetched?).num_days().max(0))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.prices.is_empty()
    }
}

/// What a launch should do about prices, as a value, so the decision can be
/// asserted without a network or an environment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Plan {
    /// `WARP_FORK_MODEL_PRICES` does not say `fetch`. **The only state in
    /// which no request is built**, and the falsifier this ticket was given.
    Off,
    /// Enabled, and the cache is younger than [`MAX_AGE_DAYS`].
    Cached { age_days: i64 },
    /// Enabled, and there is nothing usable on disk.
    Fetch,
}

impl Plan {
    /// Split out from everything that touches the world. `enabled` is
    /// [`crate::fork::model_prices_fetch`]; `fetched` is the cache's date.
    pub(crate) fn of(enabled: bool, fetched: Option<DateTime<Utc>>, now: DateTime<Utc>) -> Self {
        if !enabled {
            return Self::Off;
        }
        match fetched {
            Some(fetched) => {
                let age_days = (now - fetched).num_days().max(0);
                if age_days >= MAX_AGE_DAYS {
                    Self::Fetch
                } else {
                    Self::Cached { age_days }
                }
            }
            None => Self::Fetch,
        }
    }
}

/// One row of OpenRouter's `/api/v1/models`. Every other field is ignored.
#[derive(Debug, Deserialize)]
struct WireModel {
    id: String,
    pricing: WirePricing,
}

/// Prices arrive as decimal strings in USD **per token** (`"0.000005"`), which
/// is why they are parsed rather than deserialized as numbers, and multiplied
/// on the way in so nothing downstream has to remember the unit.
#[derive(Debug, Deserialize)]
struct WirePricing {
    prompt: String,
    completion: String,
}

#[derive(Debug, Deserialize)]
struct WireCatalogue {
    data: Vec<WireModel>,
}

/// Per million tokens, from a per-token decimal string. A row whose price does
/// not parse, or is negative, is dropped rather than drawn as zero — a free
/// model and an unparseable one must not look alike on the card.
fn per_million(value: &str) -> Option<f32> {
    let parsed: f32 = value.trim().parse().ok()?;
    (parsed >= 0.0).then_some(parsed * 1_000_000.0)
}

/// The catalogue as this module keeps it, from the body of one response.
/// Public to the tests so the shape can be pinned against a saved payload
/// without a network.
pub(crate) fn parse(body: &str, now: DateTime<Utc>) -> Result<Catalogue, serde_json::Error> {
    let wire: WireCatalogue = serde_json::from_str(body)?;
    let prices = wire
        .data
        .into_iter()
        .filter_map(|model| {
            Some((
                model.id,
                Price {
                    input: per_million(&model.pricing.prompt)?,
                    output: per_million(&model.pricing.completion)?,
                },
            ))
        })
        .collect();
    Ok(Catalogue {
        fetched: Some(now),
        source: Some(SOURCE.to_owned()),
        prices,
    })
}

/// Whether this launch has already decided. Set before the request, so a
/// failure does not retry on the next turn either: "at most one request per
/// launch" is the promise, and a network that is down would otherwise make it
/// one per turn.
static DECIDED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Bring the catalogue up to date, at most once per launch. Called from
/// `session/new`, which is also where [`super::specs::Table`] is reloaded, so
/// the card and the prices behind it are refreshed by the same event.
///
/// Every failure is a log line and the previous catalogue: no turn fails
/// because a price list did not answer.
pub(crate) async fn refresh_once() {
    if DECIDED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let cached = Catalogue::current();
    let plan = Plan::of(
        crate::fork::model_prices_fetch(),
        cached.fetched,
        Utc::now(),
    );
    match plan {
        Plan::Off => {}
        Plan::Cached { age_days } => {
            log::debug!("fork: model prices are {age_days} days old, not refetching");
        }
        Plan::Fetch => match fetch().await {
            // **An empty catalogue is dropped, not installed.** A 200 whose
            // body is `{"data":[]}` parses perfectly and would replace every
            // price the last fetch found with nothing -- turning a card that
            // said `$5 in · $25 out` into one that says `?`, from a response
            // that reported success. Keeping the old numbers and their age is
            // strictly better: the card already says how old they are.
            Ok(catalogue) if catalogue.is_empty() => {
                log::warn!("fork: {SOURCE} answered with no models; keeping the cached prices");
            }
            Ok(catalogue) => {
                log::info!(
                    "fork: fetched {} model prices from {SOURCE}",
                    catalogue.prices.len()
                );
                write_cache(&catalogue);
                Catalogue::install(&Arc::new(catalogue));
            }
            Err(error) => log::warn!("fork: model prices unavailable from {SOURCE}: {error}"),
        },
    }
}

/// The one request. Reached only from the [`Plan::Fetch`] arm above, and
/// `prices_tests.rs` pins that this is the module's only HTTP client.
async fn fetch() -> anyhow::Result<Catalogue> {
    let client =
        http_client::Client::from_client_builder(reqwest::Client::builder().timeout(TIMEOUT))?;
    let body = client
        .get_without_warp_headers(SOURCE)
        .send()
        .await?
        .text()
        .await?;
    Ok(parse(&body, Utc::now())?)
}

/// Best-effort, and a failure is a log line: the catalogue is already live in
/// this process, and losing it costs the next launch one request.
fn write_cache(catalogue: &Catalogue) {
    let dir = crate::fork::state_dir();
    if let Err(error) = crate::fork::create_private_dir(&dir) {
        log::warn!("fork: cannot create {}: {error}", dir.display());
        return;
    }
    let path = dir.join(CACHE_FILE);
    match serde_json::to_string_pretty(catalogue) {
        Ok(text) => {
            if let Err(error) = std::fs::write(&path, text) {
                log::warn!("fork: cannot write {}: {error}", path.display());
            }
        }
        Err(error) => log::warn!("fork: cannot serialize the price catalogue: {error}"),
    }
}

#[cfg(test)]
#[path = "prices_tests.rs"]
mod tests;

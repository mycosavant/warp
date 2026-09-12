# Codex as an ACP agent: the OpenRouter key fits, the telemetry does not

Asked 2026-09-11 during the decide run, from two of the maintainer's questions:
*can the OpenRouter key opencode already holds be passed to codex*, and *what is
the stock release build's telemetry posture* — named as the one off-thesis thing
they could think of.

Read against **`openai/codex` fetched live**, HEAD `53ff712a48` (2026-09-12), not
against the local checkout. That distinction is the finding's foundation and is
why the first pass would have been wrong.

## The checkout on this machine cannot answer either question

`~/git/codex` is `mycosavant/codex`, HEAD `404c126fc3`, **2025-09-15 — a full
year stale**, not the month it looks like. It was read first and it gives a
confidently wrong answer to the telemetry question: **zero** telemetry
dependencies, no opentelemetry, no sentry, no analytics crate.

**The tell that it was lying was already on disk**: `~/.codex/installation_id`,
written **2026-09-07** — by the `codex-acp` probe of that day — and **nothing in
the year-old tree writes that file**. A recent artifact with no writer in the
source you are reading is the source telling you it is not the source that ran.

## Third-party providers: yes, by config alone, no code

`codex-rs/core/src/model_provider_info.rs:258`, codex's own comment:

> We do not want to be in the business of adjudicating which third-party
> providers are bundled with Codex CLI, so we only include the OpenAI and open
> source ("oss") providers by default. Users are encouraged to add to
> `model_providers` in config.toml to add their own providers.

`ModelProviderInfo` carries `name`, `base_url`, `env_key`, `env_key_instructions`,
`wire_api`, `query_params`, `http_headers`, `env_http_headers`,
`request_max_retries`. `codex.rs:329` documents the provider id as
*`("openai", "openrouter", ...)`* — OpenRouter is codex's own second example.

opencode's stored credential is `provider "openrouter", type "api"`
(`~/.local/share/opencode/auth.json`), an OpenAI-compatible endpoint. So it is
reusable as-is:

```toml
[model_providers.openrouter]
name = "OpenRouter"
base_url = "https://openrouter.ai/api/v1"
env_key = "OPENROUTER_API_KEY"
wire_api = "chat"
```

**Untested.** This is read from source, and the three things that would settle it
are a build, a `session/new`, and one turn.

## Telemetry: present, on by default, and switchable off

Current upstream carries all three of the things the year-old tree had none of:

| | |
|---|---|
| a dedicated `codex-analytics` crate | depended on by `core`, `core-api`, `core-plugins`, `app-server` |
| OpenTelemetry | `opentelemetry`, `-appender-tracing`, `-otlp`, `-semantic-conventions`, `_sdk`, `tracing-opentelemetry`, all 0.31/0.32 |
| Sentry | `0.46.0`, plus a `feedback` crate that uploads envelopes |

**The gate is default-on.** `codex-rs/analytics/src/client.rs:247` builds its
queue when `analytics_enabled != Some(false)` — so absent config, events are
collected and sent. They POST to `{base_url}/codex/analytics-events/events`.

**The switch exists and is one config block.** `core/src/config/mod.rs:4342`
reads `cfg.analytics.as_ref().and_then(|a| a.enabled)`, so:

```toml
[analytics]
enabled = false
```

**Not yet established**: whether that key covers the OpenTelemetry and Sentry
paths too, or only the analytics queue. Three subsystems, one key read — the
arithmetic does not obviously work out, and assuming it does is the same
mistake as reading the stale checkout.

## And the ACP wrapper is a second vendor

`@zed-industries/codex-acp` is **Zed's**, not OpenAI's. Its telemetry posture is
a separate question from codex's and has not been looked at at all.

## Where this leaves it

The maintainer's direction, 2026-09-11: follow through to a build and a probe,
and **maintain a patch** that removes telemetry or loops it back to a surface
the fork owns. That surface is believed not to exist yet — possibly documented,
possibly stubbed — and finding out is its own task.

The sequencing that falls out of the above: the `[analytics] enabled = false`
block is necessary and **not demonstrated sufficient**, so the first run of a
codex binary should be watched on the wire rather than trusted to its config.

> Ticket T21, filed 2026-09-07 from the maintainer's notes after the phone
> checklist. What is marked *read* here was read, not run; the survey section
> says which is which per row.

## T21 — The agent's models: what the panel can say about them, and which agents say it

**Filed 2026-09-07.** T14.14 gave the panel's chip, `/model` and the menu the
ACP agent's own model list, measured against `claude-agent-acp` 0.73.0
(`.fork/docs/composer.md`, "The model picker, built and measured"). Using it,
the maintainer asked three things, and each is an item here:

1. the chip read a sentence, *Fable (Fable 5.1 · Most capable for your
   hardest and longest-running tasks)*: name the model, and maybe one word;
2. the *Model Specs* card beside the list shows `?` on all three bars: cost
   is published, so fill it, and say what the other two bars can honestly be;
3. how much of this is the protocol and how much is per agent: Grok, Gemini,
   Codex, opencode, OpenRouter, and upstream's own custom inference.

### How stock Warp labels and ranks a model, read 2026-09-07

Two different answers, because upstream has two kinds of model list.

**Its own agent's list** comes from Warp's server as `LLMInfo`, and the
picker draws `display_name` alone. `description` is null on every server
model in the test fixtures and is used for one thing: custom model routers,
where it reads *Custom · <endpoint>* and `menu_display_name` deliberately
keeps it off the chip. The comment on that function calls the parenthetical
form *"a temporary implementation that won't scale well for longer
descriptions"*. The three bars are `LLMSpec { cost, quality, speed }`, floats
in `0..=1` drawn by `Percentage::width`, sent by the server, and the card's
header says what they are: *"Warp's benchmarks for how well a model performs
in our harness, the rate at which it consumes credits, and task speed."* With
a BYO key the cost bar is replaced by *Inference via API key* and a Manage
button (`data_source.rs`, `CostRow::BilledToProvider`).

**A third-party harness's list** (`Harness::{Claude, OpenCode, Gemini,
Codex}`, upstream's cloud agents) is `HarnessModelInfo { id, display_name,
reasoning_level }` from the `get_available_harnesses` GraphQL query. No
description, no spec. The chip shows `display_name` and the card is not
drawn. So upstream does not rank a harness's models at all; the specs card is
its own agent's.

**Where the sentence came from.** `claude-agent-acp` names a row by family
(`Fable`) and puts the version and a tagline in the ACP `description`,
joined by ` · `. The taglines are Claude Code's own strings (four of them in
the `claude` 2.1.263 binary: *Most capable for your hardest and
longest-running tasks*, *Best for everyday, complex tasks*, *Efficient for
routine tasks*, *Fastest for quick answers*), passed through unchanged. The
`default` row's description is the model it resolves to, with no separator
(*Opus (1M context)* on 2026-09-07's list).

### T21.1 — The chip names the model ✅ **done 2026-09-07, `8a6e64f81`**

The label is the segment before ` · ` when there is one, the name otherwise,
and the description is no longer passed into `LLMInfo`. So *Fable 5.1*,
*Opus 5 with 1M context*, *Sonnet 5*, *Haiku 4.5*, *Default (recommended)*.
`opencode` sends no descriptions and is unchanged.

**Not done, and part of T21.2:** the one-word classification the maintainer
suggested, and the tagline, which now appears nowhere on screen. Both belong
on the specs card, not on the chip.

### T21.2 — The specs card for an agent's list ✅ **built 2026-09-07, `5856078d9`, `21b8d2341`**

**As built, after the maintainer agreed with the recommendation below
(table, fetch later; the fork's ranking with the vendor's ordering as the
fallback).** `app/src/ai/acp_agent/specs.rs` and `specs.default.toml`:

- `<state dir>/acp-model-specs.toml` lays over the compiled defaults by
  `id` (a row replaces, a new id adds, `as_of` and `source` carry the date
  the prices were read); re-read on every `session/new`; a malformed file
  is logged and is the defaults.
- Cost is `output` against the dearest `output` among the models the
  agent currently offers, so the same row draws differently under a
  different list. Intelligence and speed are the table's, then the tier
  the Claude Code tagline names, then `?`. A half-known spec is written
  with `-1.0` for the unknown field and the inline card draws that bar as
  `?`; the settings page clamps it to an empty bar under the same header.
- Lookup keys: the id, the id without a `[..]` suffix (`opus[1m]`), and for
  a description with no ` · ` its first word, which is how Claude Code's
  `default` row (described as *Opus (1M context)*) finds `opus`.
- One new predicate, `fork::model_list_is_the_agents`, asked by the chip
  (the sentence stays off it, as for a custom router) and by both specs
  cards (the header). The menu row shows the table's `class` after the
  name. The tagline is `LLMInfo::description` again, drawn under the
  header with the price line.
- Prices as of 2026-09-07 from the vendor's pricing page: Fable 5.1
  $10/$50, Opus 5 $5/$25, Sonnet 5 $2/$10, Haiku 4.5 $1/$5 per million
  tokens in/out. The ranking is the maintainer's sketch filled in and is
  labelled as an opinion in the file.

Measured in `.fork/runs/model-2026-09-07/` (`specs-run.sh`); the account is
in `.fork/docs/composer.md`, "The specs card". The first build clipped the
Cost row, because the inline menu's height does not follow its details
pane; the second is the one that counts.

**Still open on this item:** the settings page's empty-bar-for-unknown; the
*Full Terminal Use* tab; the fetch behind a switch, if the table goes stale
often enough to matter. None is scheduled.

#### The design, as written before the decision


**What the wire carries**: `id`, `name`, `description`, `category`, groups.
Nothing numeric. Every number on the card is therefore something the fork
supplies, and the card's header must say so, because the current header
claims Warp's benchmarks over a list Warp has never run.

**Cost is the one real number, and on a subscription it is the wrong unit.**
Vendors publish list prices per million tokens, in and out. The agent here
runs on the maintainer's Claude subscription, where a turn costs no dollars
and instead spends a rate-limit window; the ratio between models is roughly
the price ratio, but the absolute figure means nothing to the person paying.
So the bar should be **relative**: output list price divided by the dearest
model the agent currently offers, dearest at full width. That is also what
"based on all models present" means in practice, and it is why the ranking
"more-or-less stands" for a single-vendor list: with only Anthropic models
offered, relative price and relative capability move together.

**Two sources for the prices, and the recommendation is the table.**

| | compiled-in table, keyed by id pattern | one fetch of a public catalogue |
|---|---|---|
| egress | none | the fork's first outbound request to a party that is not the user's own agent. `egress.rs` is a deny-list, so it would pass by default; it should not pass without an opt-in |
| freshness | stale until a commit | fresh, cached under `fork::state_dir()` with a TTL |
| mapping | Claude Code's ids (`fable`, `opus[1m]`, `sonnet`) to a price row: a table | the same ids to the catalogue's slugs (`anthropic/claude-…`): **still a table**, only the numbers move |
| coverage | what the fork's maintainers write down | OpenRouter's `/api/v1/models` answers for ~356 models across vendors without a key, which covers `opencode` over OpenRouter as well |

The fetch saves editing numbers and costs a network request and an
opt-in variable. The mapping table exists in both designs. **Start with the
table**, in a file the person can edit without a rebuild
(`fork::state_dir()/acp-model-specs.toml`, shipped defaults compiled in,
unknown id draws `?`), and add the fetch later as `WARP_FORK_MODEL_PRICES=
fetch` if the table goes stale often enough to matter. That order keeps the
fork's "nothing leaves the machine unasked" claim intact by default.

**No prices are written in this ticket.** They have to be read off the
vendor's pricing page on the day the table is written and dated in the
file, the same as any measured claim here.

**Intelligence and speed have no published number, so say what they are.**
Three honest options, in order of preference:

1. **A ranking the fork keeps**, `0..=1` per model, in the same TOML, with
   the header reading something like *"List price relative to the dearest
   model offered, and a ranking kept in this fork. Not a benchmark."* The
   maintainer's sketch (*Fable 10 · 5 · 8, Haiku 4 · 8 · 3*) is this table.
   It is an opinion, and labelled as one it is useful.
2. **The vendor's own ordering**, derived from the tagline tier (*Most
   capable* > *Best for everyday* > *Efficient* > *Fastest*) and its inverse
   for speed. Needs no table for Anthropic models, is the vendor's claim
   rather than the fork's, and breaks the day the strings change. Worth
   using as the default when the table has no row.
3. **Leave `?`.** Honest and what the card does today.

Recommend 1 with 2 as the fallback, and the header text changed either way.
A bar under a header that claims a benchmark is the stale-doc defect drawn
as a picture.

**Also on the card, cheap:** the tagline as a line of text under the header
(the row struct in `data_source.rs` already carries `description`; the
non-router branch just never draws it), and the one-word classification
beside the name in the menu row, from the same table column. **And the
*Full Terminal Use* tab**: it reads `cli_agent`, which nothing fills. One
line in `picker.rs` fills it with the same list; whether it should is a
question about what that tab means for an agent Warp does not run, and the
answer is probably "the same list, because the chip switches to it when a
block is agent-in-control".

### T21.3 — Which agents say any of this: the survey

The protocol has **two** shapes for model selection, and the fork reads one.

- **`configOptions`**, in schema 1.5.0's stable surface:
  `SessionConfigOption { id, name, description, category, kind }` with
  `category ∈ {mode, model, model_config, thought_level, other}` and
  `kind ∈ {Select, Boolean}`, on `session/new` and `session/load` replies,
  changed by `session/set_config_option`. This is what T14.14 reads, and
  `Catalog::of` keeps only `category: "model"`.
- **`models: { availableModels, currentModelId }`** with
  `session/set_model`, the earlier draft. Still emitted by some agents. The
  fork does not read it.

| agent | version | model list on the wire | how it is set | evidence |
|---|---|---|---|---|
| `claude-agent-acp` | 0.73.0 | `configOptions`: `model` (category `model`), `effort` (`thought_level`), `agent` (no category) | `session/set_config_option` | **run**, 2026-09-07 |
| `opencode` | 1.18.25 | `configOptions` with categories `model`, `mode`, `thought_level`, `agent_context`, `conversation`, `system_prompt`, `tab`, and the `models` draft beside them | `session/set_config_option` | **run** 2026-09-07 in the panel: 365 rows, a pick sent and honoured, confirmed in opencode's own database (`.fork/runs/openrouter-2026-09-07/`) |
| `@google/gemini-cli` | 0.58.0 | the **`models` draft** (`availableModels`, `currentModelId`) and `modes`; the bundle carries the `configOptions` schema but builds no such list | `session/set_model` (`unstable_setSessionModel`) | bundle **read**; **probed** 2026-09-07: `initialize` answered, `session/new` refused for want of a Gemini credential, so the list is unseen |
| `@zed-industries/codex-acp` | 0.16.0 | unknown: a native binary per platform, README lists slash commands, permissions, auth methods and no model selection | unknown | **probed** 2026-09-07: `initialize` answered (`codex-acp` 0.16.0, protocol 1, session list/resume/close), `session/new` refused with `Authentication required`, so the list is unseen |
| Grok | — | no ACP agent for xAI found on the npm registry. Upstream's Grok support is an OAuth token sent *inside the request to Warp's backend* (`api_keys_for_request`, `grok_oauth_access_token`), the same billing-substitution path as every BYO key, which the fork never reaches | — | `crates/ai/src/api_keys.rs`, **read** |

So the honest answer to "is it plug and play": **the `configOptions` half
is, and two of the four agents speak it.** `claude-agent-acp` and `opencode`
need nothing further. Gemini needs a second door for the draft shape, which
is the same `Catalog` with a different field read and a different request
sent, small if it is wanted. Codex needs a probe before anything is said:

```
warpctrl acp probe --command "npx -y @zed-industries/codex-acp@0.16.0" --prompt "which model are you?"
```

prints the `initialize` reply and the `session/new` reply, which is where a
list would be.

Items, none started:

- [x] **T21.3a** probed 2026-09-07, `.fork/runs/acp-survey-2026-09-07/`:
      both agents answer `initialize` and refuse `session/new` without a
      credential this machine does not have (Codex: a ChatGPT login or an
      OpenAI key; Gemini: a Google login or a Gemini key). The model list is
      on the `session/new` reply, so the two rows stay half-filled until the
      maintainer supplies one. What the probes settled anyway: **both
      advertise `authMethods`, and `acp_agent` never sends `authenticate`**.
      With `claude-agent-acp` the login is Claude Code's own file and the
      panel never needed the step; with either of these the panel would fail
      every `session/new` the way the probe did. Sending `authenticate` with
      a method id is small; choosing the method is the person's, as with
      `WARP_FORK_ACP_MODE`. Filed as T21.3d below.
- [x] **T21.3b** run 2026-09-07, `.fork/runs/openrouter-2026-09-07/`: it
      already worked. 365 rows (358 OpenRouter, 7 OpenCode Zen) through
      `AvailableLLMs::new`, a pick sent as `session/set_config_option` and
      honoured, the answer and opencode's own `opencode.db` agreeing. The
      search box is unmeasured: posted characters never reach it, so the
      instrument cannot type. The specs card draws `?` on every row, as
      designed for an id the table does not know; the fetch that would fill
      365 rows is `.fork/next.html` item 4. One trap for the launch: a login
      shell started by `wsl.exe` has no nvm on its PATH, so name the agent by
      its absolute path or `initialize` closes the transport with nothing in
      the panel saying why.
- [ ] **T21.3c** the `models` draft as a second door, if gemini is wanted
      in the panel.
- [ ] **T21.3d** `authenticate`: an agent that lists `authMethods` on
      `initialize` gets the one `WARP_FORK_ACP_AUTH=<method id>` names, sent
      before `session/new`, and the panel reports the refusal in the agent's
      words when none is named. Blocked on a credential to measure against,
      not on code. Whether a login made in the agent's own CLI is found by the
      ACP process is the first thing to measure once one exists.

### T21.4 — OpenRouter, custom inference, and what the fork already has

Answered by reading on 2026-09-07, which is what I22 asked for under "Look
for the gate first":

- **Upstream already has an OpenRouter key.** `ApiKeys.open_router` in
  `crates/ai/src/api_keys.rs`, pasted on the Warp Agent settings page beside
  Anthropic, OpenAI and Google. On upstream's agent path it is sent to
  Warp's backend inside the request, so on the fork it reaches nothing
  there. The fork's `local_completion` already uses it, last in its chain,
  for next-command, titles and code review (`.fork/docs/manual.md`,
  "Setting it up").
- **Custom Inference** (Settings → Warp Agent → Custom Inference, an
  endpoint, a key and a model list) is the same: upstream's agent takes it
  through Warp's backend; the fork's `local_completion` takes it directly.
  It is not a route into the agent panel and cannot be made one, because the
  panel's agent is a process Warp does not do inference for.
- **The route into the panel for OpenRouter is `opencode` over ACP**, which
  was the credential in use when I22 was written. The maintainer's remark
  that custom inference "might also be the simplest way to add OpenRouter"
  holds for the small features and not for the agent, and T14.14's picker is
  the OpenRouter picker once T21.3b is measured.

So "OpenRouter support" splits: the small features have it; the panel has it
through `opencode` and needs a measurement; the secrets store I22 wants is
untouched by any of this and keeps its threat-model gate.

### T21.5 — A model on this machine, and the wiring that was refused (2026-09-07)

The maintainer asked which open-weight model fits the 12 GB card with the
least quantization loss, and asked for the OpenRouter and local wiring of the
small features to be checked. `.fork/runs/localmodel-2026-09-07/README.md`
holds the survey and the numbers; what belongs here is what it changed.

- **The wiring was dead for twelve days, and T21.4 above was written against
  it without noticing.** Upstream's 2026-08-26 merge added
  `validate_custom_endpoint_url` (https only, no local hosts), run on the
  modal and on every settings load. So "the fork's `local_completion` already
  uses it" was true of the code and false of the product: a loopback endpoint
  could not be saved and, declared in the file, took every endpoint with it.
  T3's tests never meet the validator. Fixed in `7529749a8`; the account is
  in `T03-small-ai-features.md`.
- **A second one beside it**: the page asks for a base URL and the fork
  posted to it verbatim, so an OpenRouter endpoint entered as the page
  says would have posted to `/api/v1`. `with_route` appends the route.
- **The runtime is llama-server on the Windows side**, reachable from WSL at
  `127.0.0.1:8080` under mirrored networking (measured both ways). Not
  Ollama and not the LM Studio already installed: neither can put a MoE's
  experts in RAM and its attention on the GPU, which is what the strongest
  model that fits needs. `C:\dev\llama\serve.ps1`.
- **Two models, one per job.** Gemma 4 12B at dynamic 4-bit for the four
  small features (7.4 GB, all on the GPU, 68 tok/s measured, no RAM beside a
  build); Qwen3.6-35B-A3B at dynamic 4-bit for the panel (22 GB, experts in
  RAM via `--n-cpu-moe`, ~38 tok/s on a 3060-class card in others' hands).
  The 27B dense models do not fit at agent speed. The Qwen is not downloaded:
  one disk, 49 GB free.
- **Thinking must be off at the server.** With it on, a `max_tokens: 64`
  one-shot returned empty `content` every time and no error.
- **Measured end to end on the Windows debug build, three of the four
  features.** Next Command, Prompt Suggestions and the commit message each
  drew from the local model within a second or two; the run table is in the
  README. **A third stale point from the same merge** was found by that run
  and fixed in `26c376090`: the reader looked at the pre-merge endpoint
  vector, which is empty once the settings file has spoken.
- **Open: the commit message in a routed WSL pane.** The daemon inside the
  distribution generates it with its own `AIClient`, in a process where the
  fork's config is not installed, so on the recommended configuration this
  one feature is dead and the log says *"No AI endpoint is configured"*.
  Unrouted, it works. Fix: the daemon returns the diff, the GUI generates; a
  protocol addition. Filed on `next.html`.

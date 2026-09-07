> Ticket T3, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

## T3 — Re-plumb the four small AI features locally  ← DONE

These are **not** on the agent or any harness — each is an independent
single-shot call to `api.warp.dev`. No streaming, no tool use, no session
state. Individually shippable.

**The mapping recorded here previously was wrong for three of the four.** It
named GraphQL mutations; all four are in fact plain REST `POST`s to `/ai/*`
with a JSON body and a JSON reply. Corrected and verified by tracing each
settings toggle to the call it actually issues:

| Toggle | Setting | Route | Method |
|---|---|---|---|
| Next Command | `IntelligentAutosuggestionsEnabled` | `/ai/generate_input_suggestions` | `ServerApi::generate_ai_input_suggestions` |
| Prompt Suggestions | `AgentModeQuerySuggestionsEnabled` | `/ai/generate_am_query_suggestions` | `ServerApi::generate_am_query_suggestions` |
| Shared Block Title Generation | `SharedBlockTitleGenerationEnabled` | `/ai/generate_block_title` | `BlockClient::generate_shared_block_title` |
| Commit & PR Generation | `git_operations_autogen_enabled` | `/ai/generate_code_review_content` | `AIClient::generate_code_review_content` |

The three mutations named before are real, but back different features:
`generate_metadata_for_command` is the workflow-metadata assistant,
`generate_commands_from_natural_language` is `#`-prefixed AI command search,
and `generate_dialogue_answer` is the legacy AI assistant panel.

- [x] **T3.1** Local completion client (own key, provider-agnostic) —
      `ai::local_completion`. Three wire protocols, chosen by the existing
      `CustomEndpointSchema`: OpenAI Chat Completions, OpenAI Responses,
      Anthropic Messages. Zero new dependencies.
- [x] **T3.2** Route `/ai/generate_block_title`
- [x] **T3.3** Route `/ai/generate_input_suggestions`
- [x] **T3.4** Route `/ai/generate_am_query_suggestions`
- [x] **T3.5** Route `/ai/generate_code_review_content`
- [x] **T3.6** Per-feature model selection, independent of the agent model —
      `agents.local_ai.models.*`, each falling back to `agents.local_ai.model`

### Seam

Four one-line branches, one per method, not a decorator on
`ServerApiProvider::get_ai_client`. That seam looked narrower — one line, and
every consumer of `Arc<dyn AIClient>` goes through it — but it cannot work:
only one of the four methods is on `AIClient`. One is on `BlockClient` and two
are inherent methods on `ServerApi`. A decorator would also have meant ~500
lines delegating the other 68 `AIClient` methods, breaking on every upstream
trait change.

### Configuration reuses what already exists

No new secret storage and no new UI. `ai::api_keys::ApiKeyManager` already
holds Custom Inference endpoints (URL + key + protocol + models) in the OS
keychain, with an editor on the Warp Agent settings page. Upstream forwards
those to `api.warp.dev` so the *server* can call the provider; the fork uses
the identical configuration to call it directly.

`settings::LocalAiSettings` therefore holds no URL and no key — only which
endpoint to use and which model per feature. `settings.toml` is plaintext, and
a test asserts no key in the group can contain `key`/`token`/`secret`/
`password`/`url`, so adding a secret-shaped setting later fails loudly.

Resolution order, most explicit first: named Custom Inference endpoint → first
Custom Inference endpoint → pasted Anthropic key → OpenAI key → OpenRouter
key. A *named* endpoint that does not exist is an error, never a fall-through
to a different provider — that would send the payload somewhere the user did
not choose. Google is absent deliberately: the Gemini API is not OpenAI-shaped
at its documented endpoint, so a Google key needs an explicit endpoint entry
rather than a guessed compatibility route.

### Fail-closed, and why it matters more here than it looks

Under fork policy these four never reach `api.warp.dev`, even unconfigured.
The payloads are the reason: terminal output plus the command that produced
it, the working directory and recent shell history, and an entire working-tree
diff. `fork::account_gate_bypassed` (T1) makes the toggles reachable without
an account, so without this a fork user could switch one on and quietly resume
shipping exactly that upstream. An unconfigured endpoint surfaces as an error
naming the setting to fill in.

### Verified

- 72 new tests, all passing. Full suite 6500 passed / 19 failed against a
  measured same-session baseline of 6428 / 19 on the stashed tree — no T3
  regressions. The 19 are pre-existing (`gh`-dependent git tests, flaky
  secret-redaction globals, terminal view); the two secret-redaction entries
  differ run to run, which is what makes them flaky rather than broken.
- Request shape asserted on the wire against a `mockito` stub through the real
  `http_client`, with full-body equality for all three protocols. This is the
  failure mode that does not announce itself: a provider ignores a field name
  it does not recognise, so a wrong `max_tokens` would surface months later as
  answers that are mysteriously short, never as an error.
- Each of the four features round-tripped end to end through that stub,
  including a fenced-JSON reply (what a small local model actually returns).
- The runtime wiring driven through a real `App`: a key added to
  `ApiKeyManager` and a per-feature model edited in settings both reach
  `config::current` without a restart. Worth its own test because a missed
  subscription fails silently — it looks like a feature that works but needs a
  relaunch, which nobody reports as a bug. Settings groups *emit* their changed
  event without calling `notify`, so `observe` would never have fired.
- Windows: 64/64 module tests pass, `warp-oss` builds and launches, and
  `warpctrl instance list` reaches it — that command runs the whole
  broker → HTTP flow, so a bare listing is end-to-end evidence that startup
  completed. `config::install` runs before the first window, so a wrong
  registration order would have been an immediate panic rather than a subtle
  fault.

**Not verified: a real provider.** Doing so needs a key, and there is no local
LLM server on this machine to substitute one. Every field name and route here
was written from the protocol, and is asserted against a stub — but a stub
agrees with whatever it is told. One real request through a configured
endpoint is worth more than all of the above, and takes a minute:

1. Settings → Warp Agent → add a Custom Inference endpoint (or paste an
   Anthropic/OpenAI key).
2. In a repo with uncommitted changes, use Commit & PR generation.

`agents.local_ai.model` overrides the model if the default is wrong for the
endpoint; a bad model name comes back as the provider's own 404, naming it.

### Fields left empty on purpose

Two response fields cannot be filled honestly from the client and are empty
rather than fabricated. `AgentModeSuggestionV2.context_block_ids` needs block
IDs the request never sends — the server resolves those from its own copy of
the session — so Next Command offers command suggestions but no agent queries.
`Suggestion::Coding` needs file locations from a server-side codebase index;
without one it would carry no files and be discarded by
`is_valid_code_delegation` anyway, so prompt suggestions are always `Simple`.

### Known: a T1 consequence, surfacing as a test failure

`ai::request_usage_model::tests::test_byo_api_key_disabled_for_anonymous_firebase_user`
fails under fork policy and passes with `WARP_FORK_POLICY=0`. It asserts
upstream behaviour that `fork::account_gate_bypassed` deliberately inverts —
BYO keys stay disabled for anonymous users. Not T3-caused; it is in the
baseline. Recorded here because it was previously counted anonymously among
"the same failure families" and deserves a name.

### Re-read 2026-09-07: the local endpoint this ticket promised was refused for twelve days

Found while checking the OpenRouter wiring for T21. Upstream's 2026-08-26
merge (`1e45ef773`, APP-5380, "Share custom inference endpoints across GUI
and TUI") added `validate_custom_endpoint_url`: https only, and every
loopback, private and link-local host refused. It runs on the modal's Save
and on every settings-file load through `CustomEndpointDefinitions`, so a
local endpoint was refused at the form and, declared in `settings.toml`,
invalidated every endpoint in the file. **The 64 tests above stayed green**,
because every one builds `CustomEndpoint` by hand and none goes through the
validator. Upstream's rule is right for upstream, whose server dials the URL;
this fork dials it from this process, so the rule is now https for a public
host and http or https for a local one, with the key optional for a local
one. Second finding in the same file: the page asks for a *base* URL and
`local_completion` posted to it verbatim, so a person following the page
posted to `/api/v1`. `with_route` appends the schema's route when missing.
Third, found by the first live run and not by reading: the same merge made
a saved endpoint a settings *definition* joined with a keychain key, resolved
by `ApiKeyManager::custom_endpoints()`, and `config::refresh` still read
`keys().custom_endpoints`, the pre-merge vector, which is empty once the
settings file has spoken. A correctly declared endpoint logged *"no Custom
Inference endpoints are configured"* twice per prompt. The install test now
declares one through the definitions door.

The "not verified against a real provider" paragraph above is answered the
same day: llama-server b10844 with Gemma 4 12B on the Windows side answers
a Next Command-shaped request at 68 tok/s, and the Warp-side run is in
`.fork/runs/localmodel-2026-09-07/`.


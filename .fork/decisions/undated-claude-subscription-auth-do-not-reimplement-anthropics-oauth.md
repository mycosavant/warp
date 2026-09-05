> A decision on record, split out of `.fork/TASKS.md` on 2026-09-05. Binding until a later decision names it.

- **Claude subscription auth: do not reimplement Anthropic's OAuth.**
  `crates/ai/src/grok_subscription/oauth.rs` proves the pattern works and is
  fully client-side (loopback PKCE, no Warp server) — but it works by reusing
  Grok-CLI's allowlisted `client_id`, i.e. Warp impersonates Grok-CLI. The
  Claude equivalent would put the subscription itself at risk, and would yield
  a bare token without the Claude Code config, MCP servers, skills, plugin or
  memory that make it useful. Drive the real `claude` binary instead — which
  is what `Harness::Claude` already does.

  Consequence: subscription → CLI harness. API key → Custom Inference. Two
  separate doors; the subscription only fits the first.

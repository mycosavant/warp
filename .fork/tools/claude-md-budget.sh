#!/bin/bash
# Fails loudly when CLAUDE.md crosses either of two thresholds, and shows
# which ## section to cut from. Exists because the prose rule in CLAUDE.md's
# own "How to write in this file" section was read and ignored twice in two
# days (2026-09-10, 2026-09-11) while the file kept growing anyway — a
# convention nobody is forced to consult is not a fix.
#
# Two tiers, because they answer different questions:
#   - CLOUD_TIER:  margin under Claude Code's own 150,000-char per-memory-file
#                  warning (max(40000, round(ctx * 0.05 * chars_per_token)) at
#                  a 1M window). This is the one Claude Code enforces itself.
#   - LOCAL_TIER:  whether a genuinely local model can answer one question at
#                  all. Most consumer-GPU local models run at 8k-32k context,
#                  not the 1M this repo's own sessions use. At that size the
#                  *fixed floor* -- Claude Code's own ~20,860-token system/tool
#                  prompt plus this file -- can exceed the whole window before
#                  a single word of conversation fits. .fork/docs/model-economy.md
#                  has the full table and the math; this number is the one
#                  row of it worth failing a gate over.
#
# Run by `script/presubmit`. Exit 0 under both budgets, 1 over either.
set -euo pipefail
cd "$(dirname "$0")/../.."

FILE="CLAUDE.md"
CLOUD_BUDGET="${CLAUDE_MD_BUDGET:-140000}"
LOCAL_CTX_TOKENS="${CLAUDE_MD_LOCAL_CTX_TOKENS:-32768}"   # a realistic local window, not this repo's 1M
SYSTEM_FLOOR_TOKENS="${CLAUDE_MD_SYSTEM_FLOOR_TOKENS:-20860}"  # measured 2026-09-09, empty cwd
CHARS_PER_TOKEN=3  # Claude's own tokenizer, per the CLI binary's threshold formula
LOCAL_BUDGET_CHARS=$(( (LOCAL_CTX_TOKENS - SYSTEM_FLOOR_TOKENS) * CHARS_PER_TOKEN / 2 ))  # leave half the remainder for conversation+answer

SIZE=$(wc -c < "$FILE" | tr -d ' ')
SIZE_TOKENS=$(( SIZE / CHARS_PER_TOKEN ))
LOCAL_USED_TOKENS=$(( SYSTEM_FLOOR_TOKENS + SIZE_TOKENS ))
LOCAL_LEFT_TOKENS=$(( LOCAL_CTX_TOKENS - LOCAL_USED_TOKENS ))

echo "$FILE: $SIZE chars (~$SIZE_TOKENS tokens)"
echo "  cloud budget: $CLOUD_BUDGET chars (margin under Claude Code's own warning)"
echo "  local budget: $LOCAL_BUDGET_CHARS chars, assuming a $LOCAL_CTX_TOKENS-token local model"
echo "    -> floor ($SYSTEM_FLOOR_TOKENS system + $SIZE_TOKENS this file) = $LOCAL_USED_TOKENS tokens, leaving $LOCAL_LEFT_TOKENS of $LOCAL_CTX_TOKENS for the conversation and answer"

FAIL=0
if [ "$SIZE" -gt "$CLOUD_BUDGET" ]; then
  echo
  echo "OVER CLOUD BUDGET by $((SIZE - CLOUD_BUDGET)) chars."
  FAIL=1
fi
if [ "$SIZE" -gt "$LOCAL_BUDGET_CHARS" ]; then
  echo
  echo "OVER LOCAL BUDGET by $((SIZE - LOCAL_BUDGET_CHARS)) chars — at a" \
       "$LOCAL_CTX_TOKENS-token local model, $LOCAL_LEFT_TOKENS tokens remain" \
       "for the whole conversation and answer. Below zero means this file" \
       "alone does not fit."
  FAIL=1
fi

if [ "$FAIL" -eq 1 ]; then
  echo
  echo "Section sizes:"
  awk '
    /^## / { if (name != "") printf "  %7d  %s\n", n, name; name=$0; n=0; next }
    { n += length($0) + 1 }
    END { if (name != "") printf "  %7d  %s\n", n, name }
  ' "$FILE" | sort -rn
  echo
  echo "Move the largest section's accounts to a .fork/docs/ page and leave"
  echo "a rule behind, the way .fork/docs/environment.md and .fork/docs/build.md"
  echo "already did. CLAUDE_MD_BUDGET, CLAUDE_MD_LOCAL_CTX_TOKENS and"
  echo "CLAUDE_MD_SYSTEM_FLOOR_TOKENS override the defaults above."
  exit 1
fi

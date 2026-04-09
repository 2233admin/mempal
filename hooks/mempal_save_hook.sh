#!/usr/bin/env bash
# mempal Stop hook — reminds the AI to save key decisions.
#
# Usage: add to ~/.claude/settings.json or project .claude/settings.local.json:
#
#   "hooks": {
#     "Stop": [{
#       "matcher": "*",
#       "hooks": [{
#         "type": "command",
#         "command": "/path/to/mempal/hooks/mempal_save_hook.sh"
#       }]
#     }]
#   }
#
# Behavior: every Nth conversation stop (default 10), returns a block
# reason that tells the AI to call mempal_ingest with the session's key
# decisions. The AI decides WHAT is worth saving — this hook only handles
# WHEN to prompt.
#
# Philosophy: the hook is optional. mempal works fine without it — the
# AI reads the memory protocol from mempal_status and should save proactively.
# This hook exists as a safety net for AIs that forget to save.

set -euo pipefail

# How often to trigger the save reminder.
INTERVAL="${MEMPAL_SAVE_INTERVAL:-10}"

COUNTER_FILE="${HOME}/.mempal/.stop_counter"
mkdir -p "$(dirname "$COUNTER_FILE")"

# Increment counter atomically
count=$(cat "$COUNTER_FILE" 2>/dev/null || echo 0)
count=$((count + 1))
echo "$count" > "$COUNTER_FILE"

# Only block every Nth stop
if (( count % INTERVAL != 0 )); then
  exit 0
fi

# Block AI with save instruction (Claude Code reads stdout as the block reason)
cat <<'EOF'
Before stopping, save the key decisions from this conversation to mempal.

For each decision:
1. Identify the decision + its rationale (not just "we used X", but "we used X because Y")
2. Call mempal_ingest with:
   - content: full decision + rationale
   - wing: current project name
   - room: (optional, let mempal auto-route)

If no new decisions were reached this round, respond "nothing to save" and stop.
EOF

# Exit 2 = block reason follows on stdout (Claude Code convention)
exit 2

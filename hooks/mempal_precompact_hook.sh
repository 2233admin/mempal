#!/usr/bin/env bash
# mempal PreCompact hook — emergency save before context compression.
#
# Usage: add to ~/.claude/settings.json or project .claude/settings.local.json:
#
#   "hooks": {
#     "PreCompact": [{
#       "hooks": [{
#         "type": "command",
#         "command": "/path/to/mempal/hooks/mempal_precompact_hook.sh"
#       }]
#     }]
#   }
#
# Behavior: fires once before Claude Code compresses the conversation context.
# Forces the AI to dump everything worth remembering to mempal BEFORE the
# compression window slides and facts are lost.
#
# This hook is more aggressive than the Stop hook — it always fires, not
# on an interval, because after compression the AI may lose access to
# earlier details.

set -euo pipefail

cat <<'EOF'
Context compression is imminent. Before the window slides, save ALL important
facts from this session to mempal:

1. Decisions made (with rationale)
2. Bugs found and fixed (with root cause)
3. Architecture choices (with alternatives considered)
4. Non-obvious gotchas or constraints discovered

For each: call mempal_ingest with content, wing, and optional room.

Prioritize decisions over narrative. Cite file paths where possible.
After saving, respond "saved N drawers" so the user can see what was persisted.
EOF

# Exit 2 = block reason follows on stdout (Claude Code convention)
exit 2

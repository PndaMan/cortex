#!/usr/bin/env bash
# Launch Cortex against a THROWAWAY, demo-populated database — for screenshots.
#
# Your real library (~/.local/share/study.cortex.app/cortex.db on Linux) is
# NEVER opened or modified: CORTEX_DATA_DIR points the app at a separate folder,
# and CORTEX_DEMO seeds that fresh DB with rich showcase data (subjects, topics,
# sources, cheatsheets, flashcards/quizzes, a full assignment board, and weeks of
# backdated study activity so the analytics dashboard is populated).
#
#   ./scripts/demo.sh          # launch (seeds once, reuses after)
#   ./scripts/demo.sh reset    # wipe the demo data and reseed from scratch
#
# When you're done, just delete the demo folder printed below.
set -euo pipefail

DEMO_DIR="${CORTEX_DEMO_DIR:-${TMPDIR:-/tmp}/cortex-demo}"

case "${1:-}" in
  reset|--reset|fresh) rm -rf "$DEMO_DIR"; echo "Wiped $DEMO_DIR — reseeding." ;;
esac

mkdir -p "$DEMO_DIR"
echo "Demo data dir: $DEMO_DIR"
echo "Your real library is untouched. Run with 'reset' to reseed."

cd "$(dirname "$0")/.."
export CORTEX_DATA_DIR="$DEMO_DIR"
export CORTEX_DEMO=1
exec bun run tauri dev

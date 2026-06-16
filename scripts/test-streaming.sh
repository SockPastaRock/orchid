#!/usr/bin/env bash
# Test streaming liveness: sends a message that triggers a large response,
# polls stream.state while in flight, and reports what was observed.

set -euo pipefail

CONVOS="$HOME/.config/orchid/conversations"
BIN="./bin/orchid"
START_TS=$(date +%s)

echo "==> Sending message..."
$BIN send "read the files src/client/anthropic.rs and src/loop/mod.rs and give me a detailed summary of each" --await &
BG=$!

# Wait for a conversation dir created after this script started
CONVO_DIR=""
for i in $(seq 1 50); do
  for dir in "$CONVOS"/*/; do
    DIR_TS=$(stat -f %m "$dir" 2>/dev/null || echo 0)
    if [ "$DIR_TS" -ge "$START_TS" ]; then
      CONVO_DIR="$dir"
      break 2
    fi
  done
  sleep 0.1
done

if [ -z "$CONVO_DIR" ]; then
  echo "FAIL: no new conversation dir found"
  wait $BG
  exit 1
fi

echo "==> Watching: $CONVO_DIR"

MAX_CHUNK=0
SAW_STATE=false

while kill -0 $BG 2>/dev/null; do
  STATE=$(cat "${CONVO_DIR}stream.state" 2>/dev/null || true)
  if [ -n "$STATE" ]; then
    SAW_STATE=true
    CHUNK=$(echo "$STATE" | awk '{print $2}')
    [ "$CHUNK" -gt "$MAX_CHUNK" ] 2>/dev/null && MAX_CHUNK=$CHUNK
    echo "$(date +%T) stream.state: $STATE"
  fi
  sleep 0.2
done

wait $BG
EXIT=$?

echo ""
echo "==> Process exited: $EXIT"

if [ -f "${CONVO_DIR}stream.state" ]; then
  echo "OK: stream.state persisted: $(cat ${CONVO_DIR}stream.state)"
else
  echo "(stream.state not present)"
fi

if $SAW_STATE; then
  echo "OK: saw stream.state active during streaming (max chunk count: $MAX_CHUNK)"
else
  echo "FAIL: stream.state never observed — streaming may not be working"
fi

echo ""
echo "==> Final conversation log:"
cat "${CONVO_DIR}conversation.jsonl"

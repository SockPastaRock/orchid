# Scripting

## Error handling

All errors are written as JSON to stderr. Exit code is `0` on success, `1` on any error.

```json
{"error":"conversation not found: fix-auth-bug"}
```

```bash
if ! orchid send --await "message" 2>/tmp/orchid-err; then
  jq -r .error /tmp/orchid-err
  exit 1
fi
```

## Patterns

### Fire-and-forget

Dispatch a run and capture the ID for later follow-up:

```bash
ID=$(orchid send "run the audit" | jq -r .id)
```

The run is already in progress. Use the ID to poll or send follow-ups:

```bash
# poll until idle
until [ "$(jq -r .status ~/.config/orchid/conversations/$ID/metadata.json)" = "idle" ]; do
  sleep 2
done

# read last assistant message
jq -r 'select(.type == "message" and .message.role == "assistant") | .message.content' \
  ~/.config/orchid/conversations/$ID/conversation.jsonl | tail -1
```

### Blocking with `--await`

```bash
ID=$(orchid create | jq -r .id)
orchid set --id $ID --working-dir /path/to/project
orchid send --id $ID --await "fix the failing test" || {
  echo "run failed"
  exit 1
}
```

### Per-project conversation

```bash
# Create and configure once
ID=$(orchid create | jq -r .id)
orchid set --id $ID --label my-project --working-dir /path/to/project --persona dev

# All subsequent sends use the ID
orchid send --id $ID --await "add a readme"
orchid send --id $ID --await "write tests for the new module"
```

Labels are for human reference only — always use the hex ID in scripts.

## jq recipes

```bash
FILE=~/.config/orchid/conversations/<id>/conversation.jsonl

jq 'select(.type == "message")'                                                          $FILE  # messages only
jq -r 'select(.type == "message" and .message.role == "assistant") | .message.content'  $FILE  # assistant text
jq 'select(.type == "tool_call") | .tool_call.calls[] | {name, input}'                  $FILE  # all tool calls
jq -s '.'                                                                                $FILE  # full history as array
```

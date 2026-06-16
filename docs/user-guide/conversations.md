# Conversations

## IDs and labels

Every conversation has an immutable random `id` (32 hex characters, e.g. `496f4a8700bae0fa3d21c8b591f67e02`). All `--id` flags and the `delete` positional argument require this hex ID.

An optional mutable `label` is a human-readable annotation for your own reference — it is surfaced in `orchid list` output but cannot be used for addressing conversations. Labels do not need to be unique.

```bash
orchid set --id <id> --label fix-auth-bug   # annotate
orchid list | jq -r '.[] | select(.label == "fix-auth-bug") | .id'  # look up the ID
```

> Installations created before version 2 may have 8-character IDs. These remain valid — do not filter by ID length in scripts.

## Listing

```bash
orchid list                  # all conversations (full metadata JSON array)
orchid list profiles
orchid list personas
```

```bash
orchid list | jq '[.[] | select(.status == "running")]'
orchid list | jq -r '.[] | select(.label == "fix-auth-bug") | .id'
orchid list | jq '[.[] | select(.working_dir == "/my/project")]'
```

## Conversation files

Each conversation lives at `~/.config/orchid/conversations/<id>/`:

| File | Purpose |
|------|---------|
| `conversation.jsonl` | Append-only message + tool event history |
| `metadata.json` | Live state — status, pid, hooks, last message |
| `orchid.log` | Run lifecycle audit trail (run_start, run_end, run_crashed, usage) |

## Run lifecycle

`metadata.json` is the authoritative state interface — no log parsing needed.

| Field | Meaning |
|-------|---------|
| `status` | `"idle"` or `"running"` |
| `pid` | PID of the active process; `null` when idle |
| `last_run_at` | Timestamp of the last completed or failed run |

If a process crashes, the next `orchid send` detects the stale `pid`, writes a `run_crashed` entry to `orchid.log`, and reconciles `status` to `idle`.

## Deleting

```bash
orchid delete <id>
```

Moves the conversation to `~/.config/orchid/conversations/.archive/<id>`. The conversation is removed from `orchid list` and can no longer be sent to. This is intentional — archival preserves history and is reversible by moving the directory back. To permanently remove a conversation, delete the directory directly:

```bash
rm -rf ~/.config/orchid/conversations/.archive/<id>
```

# CLI

## Output contract

All commands write JSON to stdout. This applies universally — including errors.

```bash
# single object
{"id":"a3f9c1b2...","label":"fix-auth-bug","created_at":"..."}

# list
[{"id":"a3f9c1b2...","label":"fix-auth-bug"},{"id":"d7e2a091...","label":"add-tests"}]

# error
{"error":"profile not found","profile":"missing-profile"}
```

No human-readable formatting is ever the default. Pipe through `jq` for filtering and display.

Streaming conversation output is intentionally not provided by the CLI — use standard tooling directly:

```bash
tail -f ~/.config/orchid/conversations/<id>/conversation.jsonl | jq .
```

See [storage.md](storage.md) for the full path layout.

---

## ID vs label

Conversations have two identifiers:

| | `id` | `label` |
|---|------|---------|
| Format | 32-char hex (e.g. `a3f9c1b2...`) | Human-readable string (e.g. `fix-auth-bug`) |
| Assigned | At creation — immutable | At creation or any time — mutable |
| Unique | Yes | No |
| Purpose | Stable reference for scripts | Human annotation only |

`--id` flags accept the hex ID only. Labels are surfaced in `orchid list` output for reference; use `jq` to look up the ID from a label before passing it to a command.

```bash
ID=$(orchid list | jq -r '.[] | select(.label == "fix-auth-bug") | .id')
```

---

## Commands

### `orchid send`

Append a user message and start the tool loop. Returns immediately after writing the user message and updating metadata — the loop runs as a background process.

```bash
orchid send --id <id> "message"
```

Options:

| Flag | Description |
|------|-------------|
| `--id` | Conversation hex ID |
| `--profile` | Override the active profile for this run |
| `--await` | Block until the turn completes, then exit |

Default behaviour is non-blocking: orchid prints the conversation metadata JSON to stdout and exits. The loop continues running as a detached background process. Observe progress with:

```bash
tail -f ~/.config/orchid/conversations/<id>/conversation.jsonl | jq .
```

With `--await`, orchid blocks until the loop completes (final assistant message or error) and exits with a non-zero status on failure.

See [execution.md](execution.md) for tool loop behaviour.

---

### `orchid list`

List resources. Returns a JSON array in all cases.

```bash
orchid list                  # all conversations with full metadata
orchid list profiles         # all profiles from config.json
orchid list personas         # all personas from config.json
```

`orchid list` returns each conversation's `metadata.json` contents as an array element. No filtering — pipe through `jq`.

```bash
# example: find conversations with a specific working dir
orchid list | jq '[.[] | select(.working_dir == "/my/project")]'

# example: list conversation labels only
orchid list | jq '[.[].label]'
```

See [conversation.md](conversation.md) for the metadata schema, [config.md](config.md) for profiles and personas.

---

### `orchid set`

Mutate persistent conversation settings.

```bash
orchid set --id <id> --persona <name>      # assign a persona
orchid set --id <id> --persona ""          # clear persona
orchid set --id <id> --label <name>        # annotate conversation
orchid set --id <id> --working-dir <path>  # set working directory
orchid set --id <id> --profile <name>      # set profile
```

All changes are written to `metadata.json`. See [conversation.md](conversation.md) for the metadata schema and [persona.md](persona.md) for the persona system.

---

### `orchid config`

Manage config.json directly. See [config.md](config.md) for schema.

```bash
orchid config use <profile>       # set current_profile
orchid config current             # print active profile name
orchid config path                # print path to config.json
```

# Sending

## Basic usage

```bash
orchid send "message"                  # fire-and-forget, prints metadata JSON
orchid send --await "message"          # block until complete
orchid send --id <id> "message"        # continue an existing conversation
```

Fire-and-forget returns immediately and runs the tool loop as a detached background process. Capture the ID for follow-up:

```bash
ID=$(orchid send "run the tests" | jq -r .id)
orchid send --id $ID --await "anything else?"
```

## Flags

| Flag | Description |
|------|-------------|
| `--id <id>` | Continue an existing conversation. Omit to create a new one. |
| `--profile <name>` | Override the active profile for this run only. |
| `--await` | Block until the loop completes. Prints post-run metadata on stdout. |

## Updating metadata

```bash
orchid set --id <id> --label <name>        # annotate conversation
orchid set --id <id> --working-dir <path>  # set working directory
orchid set --id <id> --profile <name>      # change profile
orchid set --id <id> --persona <name>      # assign a persona
```

## Stopping a run

```bash
orchid stop --id <id>
```

Sends SIGTERM to the running process and marks the conversation idle. No-op if not running. Returns updated metadata JSON.

## Built-in tools

The model has access to three tools during a run, all scoped to `working_dir`:

| Tool | Description |
|------|-------------|
| `bash` | Execute a shell command |
| `fs_read` | Read a file |
| `fs_edit` | Replace an exact string in a file |

Any path outside `working_dir` is blocked — the model receives a structured error. See [architecture/execution.md](../architecture/execution.md) for scope enforcement details.

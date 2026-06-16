# Execution

## Tool loop

`orchid send` appends the user message, forks the tool loop as a background process, and exits. The loop runs to completion independently:

1. Read `conversation.jsonl` to build message history. See [conversation.md](conversation.md).
2. Resolve the active profile and API key. See [config.md](config.md).
3. Resolve the active persona into a system prompt. See [persona.md](persona.md).
4. Send system prompt + message history + tool definitions to the model.
5. Model responds with either a `message` or a `tool_call`.
6. Append the response event to `conversation.jsonl`.
7. If `tool_call`: execute the tool, append `tool_result`, go to step 4.
8. If `message`: loop ends.

With `--await`, the calling process blocks until the loop completes instead of returning immediately. See [cli.md](cli.md).

---

## Run lifecycle

On every run transition, orchid performs two writes atomically: one to `logs.jsonl` (permanent record) and one to `metadata.json` (live state interface). These always happen together.

### On run start

1. Append `run_start` to `logs.jsonl`
2. Update `metadata.json`:
   - `status` → `running`
   - `pid` → current process PID
   - `run_started_at` → current timestamp

### On run end

1. Append `run_end` to `logs.jsonl`
2. Update `metadata.json`:
   - `status` → `idle`
   - `pid` → `null`
   - `run_started_at` → `null`
   - `last_run_at` → current timestamp

### On crash

If the process exits without completing step 2 of run end, `metadata.json` is left stale with `status: running`. Any reader — including orchid on next invocation — detects this by checking whether the stored `pid` is still alive (POSIX signal 0). On detecting a stale run:

1. Append `run_crashed` to `logs.jsonl`
2. Reconcile `metadata.json` to `status: idle`

---

## logs.jsonl event types

`logs.jsonl` is the permanent audit trail of run boundaries. It is not used for state observation — use `metadata.json` for that.

| `type` | Written by | Description |
|--------|------------|-------------|
| `run_start` | orchid on run begin | Run boundary open |
| `run_end` | orchid on run complete | Run boundary close, with status |
| `run_crashed` | orchid on next invocation | Retroactive record of a crashed run |
| `run_stopped` | orchid stop command | Record of an externally stopped run |

```jsonl
{"event_id":"e5f6a7b8c9d0e1f2","ts":"2026-04-28T10:00:00.000Z","type":"run_start","run_id":1,"pid":12345}
{"event_id":"f6a7b8c9d0e1f2a3","ts":"2026-04-28T10:00:02.001Z","type":"run_end","run_id":1,"status":"success","duration_ms":2001}
{"event_id":"a7b8c9d0e1f2a3b4","ts":"2026-04-28T10:01:00.000Z","type":"run_start","run_id":2,"pid":12346}
{"event_id":"b8c9d0e1f2a3b4c5","ts":"2026-04-28T10:01:05.000Z","type":"run_end","run_id":2,"status":"error","error":"model returned empty response"}
{"event_id":"c9d0e1f2a3b4c5d6","ts":"2026-04-28T10:05:00.000Z","type":"run_crashed","pid":12347}
{"event_id":"d0e1f2a3b4c5d6e7","ts":"2026-04-28T10:06:00.000Z","type":"run_stopped","pid":12348}
```

`run_id` is a monotonically incrementing integer scoped to the conversation. `run_end` status values:

| Status | Meaning |
|--------|---------|
| `success` | Loop completed with a final assistant message |
| `error` | Loop stopped due to a handled error |

---

## Built-in tools

| Tool | Description |
|------|-------------|
| `bash` | Execute a shell command. Path-validated against `working_dir` before execution. |
| `fs_read` | Read a file. Path-validated against `working_dir` before execution. |
| `fs_edit` | Replace an exact string in a file. Path-validated against `working_dir` before execution. |

### `bash` scope restriction

Before executing any command, orchid tokenises the command on whitespace and checks each token that looks like a path. Common shell expansions (`~`, `~/...`, `$HOME/...`, `${HOME}/...`) are expanded first. Each resulting absolute path is cleaned and checked against `working_dir`. If any path falls outside the working directory the command is not executed and a plain-string error is returned to the model as the tool result:

```
Error: path out of scope: /etc/hosts
```

This guards against accidental out-of-scope access and prompt injection attempting to reach sensitive paths. It does not prevent runtime path construction (e.g. paths computed during execution) — enforcement is static, applied to explicit tokens in the command string before the shell runs.

The subprocess is launched with its working directory set to `working_dir` so relative paths resolve naturally within scope.

`working_dir` is set in `metadata.json`. See [conversation.md](conversation.md).

---

## Observability

Events are written to `conversation.jsonl` as they occur. Observe in real time with standard tooling:

```bash
tail -f ~/.config/orchid/conversations/<id>/conversation.jsonl | jq .
```

Observe run state without log parsing:

```bash
cat ~/.config/orchid/conversations/<id>/metadata.json | jq .status
```

See [storage.md](storage.md) for path layout and [conversation.md](conversation.md) for file schemas.

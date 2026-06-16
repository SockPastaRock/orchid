# Conversation

A conversation is a directory under `~/.config/orchid/conversations/<id>/`. See [storage.md](storage.md) for the full layout.

Each conversation contains three files with distinct responsibilities:

| File | Purpose | Mutable |
|------|---------|---------|
| `conversation.jsonl` | Message and event history sent to the model | Append-only |
| `metadata.json` | Conversation state and context | Yes |
| `logs.jsonl` | Execution trace, not sent to the model | Append-only |

---

## conversation.jsonl

The canonical message history. Append-only. Each line is a JSON event.

```jsonl
{"event_id":"a1b2c3d4e5f6a7b8","ts":"2026-04-28T10:00:00.000Z","type":"message","role":"user","content":"fix the build"}
{"event_id":"b2c3d4e5f6a7b8c9","ts":"2026-04-28T10:00:01.123Z","type":"tool_call","call_id":"abc","name":"bash","args":{"cmd":"go build ./..."}}
{"event_id":"c3d4e5f6a7b8c9d0","ts":"2026-04-28T10:00:01.456Z","type":"tool_result","call_id":"abc","name":"bash","content":"..."}
{"event_id":"d4e5f6a7b8c9d0e1","ts":"2026-04-28T10:00:02.000Z","type":"message","role":"assistant","content":"The issue was a missing import."}
```

Event types:

| `type` | Description |
|--------|-------------|
| `message` | User or assistant text turn |
| `tool_call` | Model-requested tool execution |
| `tool_result` | Output returned from a tool |

`tool_call` and `tool_result` events are correlated via `call_id`.

This file is the source of truth for model context, replay, and streaming. See [execution.md](execution.md) for how it is consumed during a run.

---

## metadata.json

Persists conversation-level state and current run status. The only mutable file in a conversation directory. Written by the orchid process — never by external tools.

```json
{
  "id": "a3f9c1b2",
  "label": "fix-auth-bug",
  "created_at": "2026-04-28T10:00:00.000Z",
  "updated_at": "2026-04-28T10:05:00.000Z",
  "profile": "anthropic-default",
  "persona": "main-dev",
  "working_dir": "/Users/chris/workspace/repo/myapp",
  "env": {
    "GO111MODULE": "on",
    "APP_ENV": "development"
  },
  "status": "idle",
  "pid": null,
  "run_started_at": null,
  "last_run_at": "2026-04-28T10:05:00.000Z",
  "last_message": "The issue was a missing import."
}
```

| Field | Description |
|-------|-------------|
| `id` | Random hash. Immutable after creation. |
| `label` | Human-readable alias. Mutable. See [cli.md](cli.md). |
| `created_at` | ISO 8601. Set once at creation. |
| `updated_at` | ISO 8601. Updated on every write. |
| `profile` | Profile used at creation. Can be overridden per-run. See [config.md](config.md). |
| `persona` | Active persona name. Composed into system prompt at run time. See [persona.md](persona.md). |
| `working_dir` | Injected as working directory for tool execution. |
| `env` | Additional environment variables applied during tool execution. |
| `status` | Current run state: `idle` or `running`. See [execution.md](execution.md). |
| `pid` | PID of the active orchid process. `null` when idle. |
| `run_started_at` | ISO 8601. Set when a run begins. `null` when idle. |
| `last_run_at` | ISO 8601. Timestamp of the last completed or failed run. |
| `last_message` | Content of the last assistant text turn. Omitted if no run has completed. |
| `hooks` | Registered lifecycle hooks, keyed by event then label. See [hooks.md](hooks.md). |

`metadata.json` is the interface for external programs to observe conversation state. Reading this file is sufficient to determine whether a conversation is active — no log parsing required. See [execution.md](execution.md) for how orchid manages these fields across the run lifecycle.

---

## logs.jsonl

Execution-level trace. Append-only. Not sent to the model.

Captures runtime events: process start/stop, errors, and internal diagnostics. Kept separate from `conversation.jsonl` so message history remains clean for model consumption and replay.

```jsonl
{"event_id":"e5f6a7b8c9d0e1f2","ts":"2026-04-28T10:00:00.000Z","type":"run_start","run_id":1,"pid":12345}
{"event_id":"f6a7b8c9d0e1f2a3","ts":"2026-04-28T10:00:02.001Z","type":"run_end","run_id":1,"status":"success","duration_ms":2001}
{"event_id":"a7b8c9d0e1f2a3b4","ts":"2026-04-28T10:01:00.000Z","type":"run_start","run_id":2,"pid":12346}
{"event_id":"b8c9d0e1f2a3b4c5","ts":"2026-04-28T10:01:05.000Z","type":"run_end","run_id":2,"status":"error","error":"model returned empty response"}
```

See [execution.md](execution.md) for the full run lifecycle and event types.

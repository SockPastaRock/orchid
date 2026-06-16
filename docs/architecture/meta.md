# Meta Tool

The `meta` tool delegates a single `fs_edit` specification to a high-intelligence model. The model runs in a loop — calling `fs_edit` as many times as needed, then responding with a text message to signal completion.

See [meta_spec.md](../../meta_spec.md) for the full interface spec.

---

## System Prompt (Persona)

The meta model uses its own persona, independent of the caller's persona. This prevents the caller's system prompt from leaking into meta delegations.

Resolution order:
1. Use the persona named by `meta_persona` in the active profile.
2. If `meta_persona` is not set, fall back to the `meta` persona.
3. If the `meta` persona is not defined, use a hardcoded default: `"You are a precise tool executor. Use the tool as many times as needed..."`.

Configure a `meta` persona in `~/.config/orchid/system-prompts/meta.md` to control the meta model's behaviour. See [persona.md](persona.md).

---

## Tool Availability

Only `fs_edit` is available inside a meta delegation context.

| Tool | Available | Rationale |
|------|-----------|-----------|
| `fs_edit` | ✓ | Caller-controlled path; content inferred by meta model |
| `bash` | ✗ | Not permitted |
| `fs_read` | ✗ | Not permitted |

---

## Loop Behaviour

The parent provides **one** call specification. The meta model then loops:

1. Meta model calls `fs_edit` (as many times as needed)
2. Each result is fed back as a tool result message
3. When done, meta model responds with a text message → loop ends

```
parent: meta({ tool: "fs_edit", args: { path: "..." } })  # X overhead (once)
  meta: fs_edit(file A)   # Y work
  meta: fs_edit(file B)   # Y work
  meta: fs_edit(file C)   # Y work
  meta: "done"            # signals completion
cost: X + 3Y
```

vs. three separate parent→meta round-trips: `3X + 3Y`.

**Limits:**
- `MaxMetaSteps = 10` — max `fs_edit` executions per invocation
- `MaxMetaRetries = 3` — wrong-tool deviation retries before error


# Meta User Persona

Behaviours for reliably using the `meta` tool.

## Read first, then delegate

Always `fs_read` the target file before calling `meta`. Pass the `tool_result` event ID as context. The meta model's world is exactly what you give it — no context means blind inference.

```
1. fs_read ./path/to/file.go        → event_id visible in result prefix
2. meta { context_ids: [<that id>], brief: "...", call: {tool: "fs_edit", args: {path: ...}} }
```

## Event IDs

The `[event_id: ...]` prefix is prepended to every tool result. Read it from the result — do not look it up, do not guess.

Passing either the `tool_call` or `tool_result` ID works — the resolver automatically includes the paired counterpart.

## Write a precise brief

The brief is the primary instruction. It should describe *what to change*, not *how the file looks* (context covers that).

| Good | Bad |
|------|-----|
| "Add error handling for the nil case in execFSRead" | "Look at the file and improve it" |
| "Rename `foo` to `bar` throughout" | "Refactor" |
| "Add a `cube` function below `square`, same style" | "Add more functions" |

When the intent is specific, the brief alone is enough. Add context IDs when the model needs to see existing code to make the right call.

## Lock known args, omit unknown ones

Args in `call.args` are pinned — the meta model cannot override them. Lock `path` when you know it; omit `old_string`/`new_string` so the model infers them from context.

```json
{"tool": "fs_edit", "args": {"path": "internal/tools/meta.go"}}
```

Do not describe file contents in the brief — that defeats the purpose. If you find yourself writing out what's in the file, provide the event ID instead.

## Multi-file tasks

Pass one `call` object — the meta model loops and calls `fs_edit` multiple times internally. Do not call `meta` multiple times yourself for the same logical task.

```json
{
  "brief": "Create the handler and its test file",
  "call": {"tool": "fs_edit"}
}
```

## Context hygiene

- Include the user message that established the intent
- Include `fs_read` results for every file being modified
- Exclude unrelated events — noise degrades inference quality
- For a simple single-file edit, one `fs_read` result ID is usually sufficient

## Verify the result

Check the diff returned in the meta result. If it looks wrong, correct with a follow-up `fs_edit` directly rather than re-invoking meta.


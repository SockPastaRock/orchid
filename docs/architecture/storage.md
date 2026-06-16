# Storage Layout

All orchid state persists under `~/.config/orchid/`.

```
~/.config/orchid/
  config.json
  system-prompts/
    base.md
    developer.md
    ...
  conversations/
    <id>/
      conversation.jsonl
      metadata.json
      logs.jsonl
```

- `config.json` — provider profiles, persona definitions, and active profile selection. See [config.md](config.md).
- `system-prompts/` — composable Markdown prompt fragments. See [persona.md](persona.md).
- `conversations/` — one directory per conversation, named by ID. See [conversation.md](conversation.md).

## Notes

- No state is stored in the project working directory.
- The `<id>` is a random hash assigned at creation and never changes. See [cli.md](cli.md) for ID vs label resolution.

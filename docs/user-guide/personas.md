# Personas

A persona is a named list of system prompt fragments composed in order at runtime. Fragments are Markdown files in `~/.config/orchid/system-prompts/`.

```json
{
  "personas": {
    "dev": ["base", "developer", "concise"]
  }
}
```

`base.md`, `developer.md`, and `concise.md` are concatenated to form the system prompt. If no persona is set, no system prompt is sent.

```bash
orchid list personas              # list defined personas
orchid send --persona dev "..."   # use a persona for this conversation
orchid set --id <id> --persona dev   # assign to an existing conversation
orchid set --id <id> --persona ""    # clear persona
```

See [architecture/persona.md](../architecture/persona.md) for how fragments are resolved.

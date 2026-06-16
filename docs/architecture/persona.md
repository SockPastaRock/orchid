# Personas

A persona is a named, ordered list of system prompt references that are composed into a single system prompt at runtime.

---

## System prompts

System prompts are Markdown files stored in `~/.config/orchid/system-prompts/`. Each file is a self-contained, composable prompt fragment named by its role.

```
~/.config/orchid/system-prompts/
  base.md
  architect.md
  concise.md
  security.md
  developer.md
```

Files are referenced by name without the `.md` extension.

---

## Personas in config.json

Personas are defined in `config.json` under a `personas` key. Each persona is an ordered list of system prompt names. See [config.md](config.md) for the full config schema.

```json
{
  "personas": {
    "main-dev": ["base", "concise", "developer"],
    "security-review": ["base", "security", "concise"],
    "architect": ["base", "architect"]
  }
}
```

Order is significant — prompts are concatenated in the order listed with a blank line between each.

---

## Composition

At run time, orchid reads each referenced `.md` file in order and concatenates them into a single system prompt string:

```
<contents of base.md>

<contents of concise.md>

<contents of developer.md>
```

If a referenced prompt file does not exist, orchid exits with a JSON error before making any API call.

---

## Assigning a persona to a conversation

A persona is set per-conversation and persisted in `metadata.json`. See [conversation.md](conversation.md) for the metadata schema.

```bash
orchid set --id <id|label> --persona <name>
```

Once set, the persona is used for every subsequent run on that conversation unless changed or cleared.

```bash
orchid set --id <id|label> --persona ""    # clear persona
```

---

## Resolution order

When a run starts, the system prompt is resolved as follows:

1. Read `persona` from the conversation's `metadata.json`.
2. Look up the persona definition in `config.json`.
3. For each name in the list, read `~/.config/orchid/system-prompts/<name>.md`.
4. Concatenate with blank line separators.
5. Pass the result as the system prompt for the run.

If no persona is set on the conversation, no system prompt is sent.

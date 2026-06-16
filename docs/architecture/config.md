# Config

Configuration is stored at `~/.config/orchid/config.json`. See [storage.md](storage.md) for the full directory layout.

## Schema

```json
{
  "current_profile": "anthropic-default",
  "profiles": {
    "anthropic-default": {
      "provider": "anthropic",
      "base_url": "https://api.anthropic.com",
      "api_key": "env.ANTHROPIC_API_KEY",
      "model": "claude-sonnet-4-6",
      "max_steps": 200
    }
  },
  "personas": {
    "main-dev": {
      "prompts": ["base", "concise", "developer"],
      "disabled-tools": []
    },
    "no-edit": {
      "prompts": ["base", "concise", "tool-use"],
      "disabled-tools": ["fs_edit"]
    }
  }
}
```

## Profile fields

| Field | Description |
|-------|-------------|
| `provider` | Internal adapter: `anthropic` only |
| `base_url` | API endpoint — use to point at a custom or proxied endpoint |
| `api_key` | `env.<VAR>` reference, resolved from environment at runtime |
| `model` | Default model for this profile |
| `meta_model` | Model used by the meta tool for delegated inference |
| `meta_persona` | Persona used as the meta model's system prompt (default: `meta`) |
| `max_steps` | Maximum tool loop iterations per turn (default: 200) |
| `max_tokens` | Maximum output tokens per model response (default: 8192). |

## `env.` resolution

Any value prefixed with `env.` is an environment variable reference resolved at runtime:

- `"env.ANTHROPIC_API_KEY"` → reads `$ANTHROPIC_API_KEY` from the process environment
- If the variable is unset, orchid exits with a JSON error before making any API call
- Literal key values are not supported — keys must always use `env.` indirection

## Active profile

`current_profile` names the profile used for all runs unless overridden. Changed via `orchid config use <profile>`. See [cli.md](cli.md).

## Personas

`personas` is a map of name to persona object. Each persona object has:

| Field | Description |
|-------|-------------|
| `prompts` | Ordered list of system prompt fragment names to compose |
| `disabled-tools` | Tools to remove from the model's tool list for this persona |

Prompt fragments are loaded from `~/.config/orchid/system-prompts/<name>.md`. See [persona.md](persona.md) for the full persona system.

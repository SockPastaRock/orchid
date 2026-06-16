# Configuration

Config file: `~/.config/orchid/config.json`

Minimal working config:

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
  }
}
```

- `api_key` must use `env.<VAR>` indirection — no literal secrets in config.
- `provider` must be `anthropic`.
- `max_steps` is the maximum tool loop iterations per turn (default: 200).

## Managing profiles

```bash
orchid config use <profile>     # set active profile
orchid config current           # print active profile name
orchid config path              # print path to config.json
orchid list profiles            # list all profiles as JSON
```

See [architecture/config.md](../architecture/config.md) for the full schema.

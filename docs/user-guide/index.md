# orchid — User Guide

orchid is a headless CLI for running LLM conversations. Every command writes JSON to stdout, the conversation is an append-only log file, and execution is a background process you observe with standard tooling.

| Doc | Topic |
|-----|-------|
| [installation.md](installation.md) | Build and install |
| [configuration.md](configuration.md) | Profiles, API keys, config file |
| [personas.md](personas.md) | System prompt composition |
| [sending.md](sending.md) | `orchid send` flags and workflow |
| [conversations.md](conversations.md) | IDs, labels, files, run lifecycle |
| [hooks.md](hooks.md) | Lifecycle hooks for automation and notification |
| [scripting.md](scripting.md) | Integration patterns, error handling, jq recipes |

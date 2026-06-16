# Project

## What this is

A minimal headless CLI that provides conversation state and tool-loop execution for LLM agent interactions. All orchestration, filtering, and querying is delegated to Unix tooling (`jq`, `bash`, `tail`). Nothing more.

---

## Package layout

```
orchid/
├── src/
│   ├── main.rs             # CLI entry point
│   ├── cli/                # Argument parsing, output formatting
│   ├── cmd/                # Command handlers (send, set, list, delete, …)
│   ├── config/             # config.json load and resolve
│   ├── convo/              # Conversation store, ID generation, resolve
│   ├── provider/           # Provider trait and shared types
│   ├── client/             # Base HTTP client, Anthropic client
│   ├── tools/              # bash, fs_read, fs_edit, scope validation
│   ├── loop/               # Tool loop, lifecycle, history, budget
│   ├── log/                # JSONL append/read, diagnostic logger
│   └── types.rs            # Shared data types
├── docs/                   # Architecture and design documentation
└── bin/                    # Compiled binary (gitignored)
```

---

## Runtime storage

All state persists under `~/.config/orchid/`. See [storage.md](storage.md) for the full layout.

```
~/.config/orchid/
├── config.json
├── system-prompts/         # composable Markdown prompt fragments
└── conversations/
    └── <id>/
        ├── conversation.jsonl    # append-only message + event history
        ├── metadata.json         # label, working_dir, env, profile, persona, timestamps
        └── logs.jsonl            # execution trace (run_start/run_end, errors)
```

---

## Key constraints

- `conversation.jsonl` and `logs.jsonl` are strictly append-only — never rewrite or truncate
- `metadata.json` is the only mutable file per conversation
- No daemon, no scheduler — blocking or backgrounded via `&`
- All CLI output is JSON to stdout — no human-readable formatting
- No CLI filtering — output raw JSON and compose with `jq`
- API keys never stored in config — use `env.<VAR>` references resolved at runtime
- No built-in streaming command — use `tail -f <conversation.jsonl> | jq .`
- ~5k LOC target — stay small

---

## CLI commands

```bash
orchid send --id <id> "message"

orchid list                  # all conversations (full metadata)
orchid list profiles         # all profiles
orchid list personas         # all personas

orchid set --id <id> --persona <name>
orchid set --id <id> --label <name>
orchid set --id <id> --working-dir <path>

orchid config use <profile>
orchid config current
orchid config path
```

See [cli.md](cli.md) for the full output contract and ID vs label resolution.

---

## Build

Always use the Makefile. Standard targets:

```bash
make build    # compile binary to ./bin/orchid
make clean    # remove ./bin/
make test     # run tests
make lint     # run linter
make check    # lint + test
```

The binary must always be built to `./bin/orchid`. Use `make build`, not `cargo build` directly.

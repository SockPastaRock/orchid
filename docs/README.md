# orchid

> Headless, composable CLI for LLM conversations.

See [user-guide/](user-guide/index.md) for the full usage reference and [architecture/](architecture/index.md) for design documentation.

## Install

```bash
make build    # compiles to ./bin/orchid
```

## Usage

```bash
# configure a profile
orchid config use anthropic-default

# send a message (non-blocking — returns id immediately)
ID=$(orchid send "fix the failing test" | jq -r .id)

# send and block until complete
orchid send --id $ID --await "fix the failing test"

# continue an existing conversation
orchid send --id <id> "follow up message"

# stream events in real time
tail -f ~/.config/orchid/conversations/<id>/conversation.jsonl | jq .

# check run state
jq .status ~/.config/orchid/conversations/<id>/metadata.json
```

## Design

- 1 conversation = 1 directory under `~/.config/orchid/conversations/`
- Tool loop execution: read log → call model → execute tools → append results → repeat
- Stream-first: observe with `tail -f` and standard tooling
- Anthropic provider only

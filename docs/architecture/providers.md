# Providers

## Structure

```
internal/provider/          # interface definition and shared types
internal/client/
  base/                     # base client: shared implementation of the provider interface
  utils/                    # shared utilities (request building, error parsing, retries, etc.)
  anthropic/                # Anthropic-specific implementation
```

---

## internal/provider

Defines the `Provider` interface and all shared data types. This is the contract between the tool loop (`internal/loop`) and any provider client. Neither the loop nor the clients depend on each other — only on this package.

```go
type Provider interface {
    Send(ctx context.Context, req Request) (Response, error)
}

type Request struct {
    SystemPrompt string
    Messages     []Message
    Tools        []ToolDefinition
}

type Response struct {
    Message   *Message    // set if model returned a text turn
    ToolCalls []ToolCall  // set if model requested tool execution; may be multiple
}

type Message struct {
    Role    string // "user" | "assistant"
    Content string
}

type ToolDefinition struct {
    Name        string
    Description string
    Parameters  any // JSON schema
}

type ToolCall struct {
    CallID string
    Name   string
    Args   map[string]any
}
```

All types are provider-agnostic. Provider clients map to and from these types internally.

---

## internal/client/base

Implements `Provider` against a normalised HTTP interface. Provider-specific clients embed `base.Client` and override only what differs — request serialisation, response parsing, auth.

Responsibilities:
- HTTP request lifecycle (send, read response, close body)
- Context cancellation and timeout propagation
- Calling into `internal/client/utils` for retries and error classification

`base.Client` is not used directly — it is always embedded in a provider-specific client.

---

## internal/client/utils

Shared utilities used across provider clients. Contains no provider-specific logic.

- Retry logic with exponential backoff
- HTTP error classification (rate limit, auth failure, transient)
- Request/response logging hooks
- `env.` key resolution (reads API key from environment)

---

## Provider clients

Each provider client lives in its own package and has one responsibility: translate between the orchid-internal types from `internal/provider` and the wire format of that specific provider's API.

### internal/client/anthropic

- Embeds `base.Client`
- Maps `Request` → Anthropic Messages API request shape
- Maps Anthropic response → `Response`
- Handles Anthropic-specific fields: `anthropic-version` header, `input_schema` tool format, `stop_reason` classification

---

## Client selection

At run time, orchid reads the `provider` field from the active profile in `config.json` and instantiates the corresponding client:

| `provider` value | Client instantiated |
|-----------------|-------------------|
| `anthropic` | `internal/client/anthropic` |

The instantiated client is passed to `internal/loop` as a `Provider`. The loop has no knowledge of which provider is in use. See [config.md](config.md) for profile configuration and [execution.md](execution.md) for how the loop consumes the provider.

# Provider Trait

No changes needed. The trait already abstracts over providers.

**Location:** `src/provider/mod.rs`

```rust
pub trait Provider: Send + Sync {
    fn send(&self, system: String, messages: Vec<Message>) -> Result<Response, ProviderError>;
    fn send_streaming(&self, system: String, messages: Vec<Message>)
        -> Result<Box<dyn Iterator<Item = Result<StreamEvent, ProviderError>>>, ProviderError>;
}
```

## What the run loop consumes

- `send()` / `send_streaming()` — the only methods the run loop (`loop/mod.rs`) calls
- `StreamEvent` — enum with `TextDelta`, `ToolCallDelta`, `Complete`
- `Response` — contains `message`, `tool_calls`, `usage`, `model`

## What each provider must do

1. Accept `system` + `messages` in the trait's format
2. Translate to the provider's wire format (HTTP request)
3. Parse the response back into `Response` (non-streaming) or `StreamEvent` (streaming)

## Existing implementation

- [`client/anthropic.rs`](../src/client/anthropic.rs) — full example. Mirror this structure for OpenAI.

## Key insight

The run loop doesn't know or care about JSON shapes, HTTP endpoints, or SSE formats. It only sees `StreamEvent::TextDelta`, `StreamEvent::ToolCallDelta`, and `StreamEvent::Complete`. That's the contract.

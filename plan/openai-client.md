# OpenAI Client Implementation

## File: `src/client/openai.rs`

Mirror the structure of [`client/anthropic.rs`](../src/client/anthropic.rs).

## Structure

```rust
struct OpenAiClient {
    base_client: BaseClient,
    api_url: String,        // e.g. http://localhost:1234/v1/chat/completions
    api_key: String,
    model: String,
    max_tokens: u32,
    extra_headers: Vec<(String, String)>,
}
```

Same fields as `AnthropicClient`. `from_profile()` resolves identically.

## Request body — `build_request_body()`

Map Anthropic format → OpenAI format:

| Anthropic | OpenAI |
|---|---|
| `system` (string) | `messages: [{role: "system", content}]` |
| `messages: [{role, content}]` | `messages: [{role, content}]` (user/assistant same) |
| `messages: [{role, tool_calls: [...]}]` | `messages: [{role, tool_calls: [{id, name, arguments}]}]` |
| `messages: [{role, tool_result: {...}}]` | `messages: [{role: "tool", tool_call_id, content}]` |
| `"tools": [...]` | `"tools": [{type: "function", function: {...}}]` |
| `max_tokens` | `max_tokens` (same field) |
| `stream: true` | `stream: true` (same field) |

**Tool schema mapping** (Anthropic → OpenAI):

```json
// Anthropic
{"name":"bash","input_schema":{"type":"object","properties":{"cmd":{"type":"string"}},"required":["cmd"]}}
// OpenAI
{"type":"function","function":{"name":"bash","parameters":{"type":"object","properties":{"cmd":{"type":"string"}},"required":["cmd"]}}}
```

## Response parsing — `parse_response()`

Non-streaming: `choices[0].message` instead of `content` array.

- `message.content` (string) → `Response.message`
- `message.tool_calls` (array) → `Response.tool_calls`
  - Map `{id, name, arguments}` → `{id, name, input: JSON.parse(arguments)}`
- `usage` → `Response.usage` (same shape)

## Streaming — `OpenAiStream`

Parse OpenAI SSE events instead of Anthropic's:

| Anthropic event | OpenAI equivalent |
|---|---|
| `message_start` | skipped |
| `content_block_start {type:"tool_use"}` | skipped (tool name from delta) |
| `content_block_delta {delta:{text}}` | `delta:{content}` → `TextDelta` |
| `content_block_delta {delta:{partial_json}}` | `delta:{tool_calls:[{function:{arguments:""}}]}` → `ToolCallDelta` |
| `message_delta {usage}` | `delta:{usage}` |
| `message_stop` | `finish_reason:"stop"` or `content:"[DONE]"` |

**Accumulator state** (same pattern as `SseStream`):
- `text_buf: String` — accumulates text deltas
- `tool_calls: Vec<ToolCallAccumulator>` — accumulates function call arguments
  - `index`, `name`, `arguments_json: String`

## Wire format details

**Endpoint:** `{profile.base_url}/v1/chat_completions` (or `/v1/chat/completions`)
**Header:** `Authorization: Bearer {api_key}` (or `api_key` from profile)
**No** `anthropic-version` header.

## Testing

See [`testing-plan.md`](./testing-plan.md).

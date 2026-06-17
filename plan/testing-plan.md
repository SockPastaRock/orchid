# Testing Plan

## Unit tests (in `openai.rs`)

Mirror the existing test patterns in [`client/anthropic.rs`](../src/client/anthropic.rs):

| Test | Purpose |
|---|---|
| `test_to_openai_message_plain` | Verify plain user/assistant messages map correctly |
| `test_to_openai_message_tool_call` | Verify tool call messages map to OpenAI format |
| `test_to_openai_message_tool_result` | Verify tool result messages map to `role: "tool"` |
| `test_openai_response_deserialization` | Verify non-streaming response parsing |
| `test_tool_schema_mapping` | Verify Anthropic → OpenAI tool schema conversion |

## Integration test (in `client/mod.rs` or separate file)

Add an `OpenAiMockProvider` following the existing `MockProvider` pattern in [`loop/mod.rs:338-364`](../src/loop/mod.rs):

```rust
struct OpenAiMockProvider { responses: Arc<Mutex<Vec<Response>>> }
impl Provider for OpenAiMockProvider {
    fn send(...) { /* same pattern as MockProvider */ }
    fn send_streaming(...) { /* same pattern */ }
}
```

Reuse existing `loop` tests — they should pass unchanged since they operate on `Provider` trait, not concrete types.

## Manual testing

1. Start LM Studio server: `lm-studio server --host 0.0.0.0 --port 1234`
2. Configure profile: `orchid config use lm-studio`
3. Create + send: `orchid create --persona default` → `orchid send --await "hello"`
4. Verify streaming output and tool calls work

## Regression check

Run existing tests after adding the file:

```bash
cargo test
```

Expected: all existing tests pass (no changes to `loop`, `cli`, `cmd`, `convo`, `tools`).

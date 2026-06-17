# Orchid OpenAI Provider — Plan

## Goal

Add LM Studio / OpenAI-compatible server support to orchid with minimal changes to existing code.

## Approach

Add a sibling module (`client/openai.rs`) that implements the existing `Provider` trait. The run loop, CLI, tools, and storage remain untouched.

## Files Changed

| File | Action | Lines |
|---|---|---|
| `client/mod.rs` | Edit | ~5 |
| `client/openai.rs` | New | ~250 |

## Design Decisions

- **No base class.** Rust traits + `BaseClient` (composition) are sufficient.
- **No shared streaming parser.** Each provider parses its own wire format (~20-30 lines per provider). The run loop never sees streaming formats.
- **`Provider` trait is the abstraction.** `send()` and `send_streaming()` are the only surface area the run loop cares about.

## File Contents

- [Provider trait](./provider-trait.md) — interface definition (no changes needed)
- [OpenAI client](./openai-client.md) — implementation details
- [Config changes](./config-changes.md) — profile wire format
- [Testing plan](./testing-plan.md) — what to verify

## Risk Assessment

| Risk | Level | Mitigation |
|---|---|---|
| Streaming parser bugs | Medium | Mirror `SseStream` structure exactly; add integration test against LM Studio |
| Tool schema mismatch | Low | OpenAI `parameters` → Anthropic `input_schema` is a simple mapping |
| Feature creep | Low | Scope: Anthropic + OpenAI only. No abstraction layer. |

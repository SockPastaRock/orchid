# Hooks

> **Not implemented.** The hooks system described here is a design sketch. The `hooks` field exists in `metadata.json` but is not read or acted on. No `hook` CLI commands exist. Do not configure hooks — they will have no effect.

---

## Planned design

Hooks would be user-defined executables called at conversation lifecycle events (`on_run_start`, `on_run_end`, `on_run_crashed`). They would be stored in `metadata.json` and fired from the lifecycle functions in `src/loop/lifecycle.rs`.

If hooks are implemented, this document will be updated with the full specification.

# Token Efficiency

Techniques for reducing LLM API token consumption in orchid. Each doc covers a single technique.

## Token cost model

| Category | Cost | Examples |
|----------|------|---------|
| Remote — input | Billed per token | History, system prompt, tool results |
| Remote — output | Billed per token (higher rate) | Model responses, tool call arguments |
| Local — compute | CPU/memory only, free | Embeddings, local model inference, vector search |

Techniques split into two categories:

- **Compression** — reduce what is sent to the remote model
- **Local substitution** — replace remote token spend with free local compute

Local substitution has a lower cost floor than compression. Both are worth pursuing.

## Techniques

| Technique | Category | Expected impact |
|-----------|----------|----------------|
| [Context resolution](context-resolution.md) | Compression | High |
| [Tool result truncation](tool-result-truncation.md) | Compression | High |
| [Rolling summarisation](rolling-summarisation.md) | Compression | High |
| [Blurring](blurring.md) | Compression | High |
| [Diff-only file reads](diff-only-file-reads.md) | Compression | Medium |
| [Repo map](repo-map.md) | Compression | Medium |
| [Structured task memory](structured-task-memory.md) | Compression | Medium-high |
| [Multi-stage model routing](multi-stage-model-routing.md) | Compression | Medium |
| [Local RAG on history](local-rag.md) | Local substitution | High |
| [Local background model](local-background-model.md) | Local substitution | Medium-high |

## Measurement

Phase 1 of this work added `orchid usage --id <id>` which reports per-step input/output token counts with a cumulative input column. This is the baseline measurement tool for validating any technique implemented here.

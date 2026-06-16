# Structured Task Memory

**Category:** Compression  
**Expected impact:** Medium-high  
**Risk:** Medium

## Problem

Task state — what the goal is, what has been decided, what has been tried — accumulates as prose spread across conversation history. On every subsequent run the full history must be re-sent to re-establish this context, even if the actual state could be expressed in a few hundred tokens.

## Approach

Introduce a structured `task_state` field in `metadata.json`. The model maintains it via a dedicated tool. On each run, only `task_state` is injected as context for prior state — not the conversation history that produced it.

```json
{
  "task_state": {
    "objective": "fix the compaction bug in index.go",
    "decisions": ["use binary search over linear scan", "preserve existing API"],
    "attempts": ["tried early return on nil key — caused regression in TestCompact"],
    "blocked_on": null,
    "recent_files": ["internal/db/index.go", "internal/db/compact.go"]
  }
}
```

## Why this is different from summarisation

[Rolling summarisation](rolling-summarisation.md) compresses history retroactively. Structured task memory is proactive — state is written in structured form as it is produced, so it never needs to be recovered from prose. The two are complementary.

## Signal to look for

High input token counts on the first step of a new run on an ongoing task — context re-establishment cost from prior history.

## Notes

- Requires the model to reliably call the update tool; prompt discipline matters
- Schema should be defined and documented so the model has clear expectations
- Risk is losing nuance that prose captures but structured fields don't — keep an `attempts` or `notes` free-text field as an escape valve

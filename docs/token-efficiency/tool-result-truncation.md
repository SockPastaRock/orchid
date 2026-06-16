# Tool Result Truncation

**Category:** Compression  
**Expected impact:** High  
**Risk:** Low

## Problem

Large `bash` outputs — compiler errors, test results, grep output — enter the full conversation history and are re-sent to the model on every subsequent loop step. A single large tool result can add thousands of tokens that persist for the lifetime of the conversation.

## Approach

Cap tool result content at a configurable byte limit before it is appended to history. Append a structured suffix so the model knows truncation occurred and can request more if needed:

```
[truncated: 3,847 lines omitted — full output available via bash if needed]
```

The limit should be configurable per profile or per conversation so it can be tuned for different workloads.

## Signal to look for

A spike in the `cumulative-input` column of `orchid usage` at a specific step, where input tokens roughly double or more. That step likely contains a large tool result that entered context.

## Notes

- The truncation suffix is important — it tells the model what happened and gives it agency to retrieve the rest
- Exit codes and the last N lines are usually more signal-dense than the first N lines; truncation should prefer keeping the tail
- This is the lowest-risk technique: it only removes noise, not reasoning

# UX Patterns

Patterns distilled from hands-on use. Each section explains not just what to do but why the approach is worth locking in.

---

## Always capture the ID at creation time

```bash
ID=$(orchid send "run the tests" | jq -r .id)
```

**Why:** The ID is the only stable, unique handle for a conversation. Labels are mutable, non-unique, and currently not accepted by `--id` flags. Capture the ID from the first command and thread it through everything that follows. Never rely on a label as an addressing mechanism in scripts.

---

## Use `create` + `set` before the first `send`

```bash
ID=$(orchid create | jq -r .id)
orchid set --id $ID --working-dir /path/to/project --persona dev --label my-project
orchid send --id $ID --await "initialise the project"
```

**Why:** `orchid send` without `--id` creates a new conversation with defaults — no persona, no working directory, no label. Separating creation from configuration makes it explicit what context the model will run in, and avoids a first turn where the model is unscoped.

---

## Fire-and-forget, then block on a specific follow-up

```bash
ID=$(orchid send "run the full test suite" | jq -r .id)
# ... do other work ...
orchid send --id $ID --await "summarise any failures"
```

**Why:** Fire-and-forget is useful when a task takes a while and you have other work. The background process writes all events to `conversation.jsonl`. When you're ready to engage, send a follow-up with `--await` to block and read the result. The model's prior tool outputs are already in the history.

---

## Stream events to monitor long-running tasks

```bash
tail -f ~/.config/orchid/conversations/$ID/conversation.jsonl | jq .
```

**Why:** The JSONL file is append-only and written in real time. `tail -f` with `jq` gives you a live stream of every message and tool call without polling the process. Combine with `jq` selectors to filter to just assistant messages or tool results.

---

## Poll for completion without an `--await` re-send

```bash
until [ "$(jq -r .status ~/.config/orchid/conversations/$ID/metadata.json)" = "idle" ]; do
  sleep 2
done
```

**Why:** When you've already dispatched a run and just need to know when it's done, read `metadata.json` directly — no need to send another message. The `status` field transitions to `idle` when the loop exits. This is cheaper than re-sending and avoids appending an empty turn.

---

## Check the conversation before sending to a running one

```bash
STATUS=$(jq -r .status ~/.config/orchid/conversations/$ID/metadata.json)
if [ "$STATUS" = "running" ]; then
  echo "conversation is running — wait before sending"
  exit 1
fi
orchid send --id $ID --await "next instruction"
```

**Why:** Sending to a running conversation causes a race on the metadata file and produces a confusing OS-level error rather than a clean guard. The safe pattern is to always check `status` first and gate the send on `idle`.

---

## Use `orchid list` + `jq` to find conversations by label

```bash
ID=$(orchid list | jq -r '.[] | select(.label == "fix-auth-bug") | .id' | head -1)
```

**Why:** Labels are not unique and not addressable directly. Treat them as annotations — use `jq` to do the lookup and extract the hex ID, then use that ID for all subsequent commands. The `head -1` guards against accidental duplicates.

---

## Set `working_dir` to scope the model's tool access

```bash
orchid set --id $ID --working-dir /path/to/project
```

**Why:** The model's `bash`, `fs_read`, and `fs_edit` tools are all gated on `working_dir`. Without it, the model cannot read or modify files. Setting it explicitly also prevents accidental scope creep — the model cannot escape the directory boundary.

---

## Use personas to control tool access and prompt composition

```bash
ID=$(orchid create | jq -r .id)
orchid set --id $ID --persona no-edit
```

**Why:** Personas are the right lever for restricting what a model can do. `disabled-tools` in the persona definition removes tools from the model's toolkit entirely — it cannot call them even if it tries. This is more reliable than prompt-level instructions alone.

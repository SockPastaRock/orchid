# Portkey Gateway Integration

Findings from probing the internal CBA Portkey gateway at
`https://d-s-genai-pkgw-alb.dotdtetp01dev01.aws.test.au.internal.cba`.

## Working request format

### Headers

| Header | Value |
|--------|-------|
| `Authorization` | `Bearer $AIPE_PORTKEY_AUTH_TOKEN` |
| `x-portkey-provider` | `anthropic` |
| `x-portkey-api-key` | `$VIRTUAL_AIPE_PORTKEY_AUTH_TOKEN` |
| `anthropic-version` | `2023-06-01` |
| `Content-Type` | `application/json` |

Two env vars are required:
- `AIPE_PORTKEY_AUTH_TOKEN` — the gateway authentication token (bearer)
- `VIRTUAL_AIPE_PORTKEY_AUTH_TOKEN` — the Portkey virtual key that routes to the backend

### Confirmed working models

Models use the `@<virtual-provider>/<model-id>` Portkey syntax:

| Model string | Backend |
|---|---|
| `@bedrock-eus1/us.anthropic.claude-sonnet-4-6` | Claude Sonnet 4.6 via Bedrock EUS1 |
| `@bedrock-eus1/us.anthropic.claude-opus-4-6-v1` | Claude Opus 4.6 via Bedrock EUS1 |

### Endpoint

```
POST /v1/messages
```

### Minimal working curl

```bash
curl -X POST "https://d-s-genai-pkgw-alb.dotdtetp01dev01.aws.test.au.internal.cba/v1/messages" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $AIPE_PORTKEY_AUTH_TOKEN" \
  -H "x-portkey-provider: anthropic" \
  -H "x-portkey-api-key: $bkAIPE_PORTKEY_AUTH_TOKEN" \
  -H "anthropic-version: 2023-06-01" \
  -d '{"model":"@bedrock-eus1/us.anthropic.claude-sonnet-4-6","messages":[{"role":"user","content":"hello"}],"max_tokens":64}'
```

## What doesn't work

| Attempt | Result | Reason |
|---------|--------|--------|
| `x-api-key` header (Anthropic native) | 401 | Gateway rejects this auth scheme |
| `Authorization: Bearer` without `x-portkey-provider` | 400 | Provider header required |
| `x-portkey-virtual-key` with either pk- key | 400 | Neither key is a virtual key |
| `x-portkey-provider: portkey_aipe` | 400 | Not a valid Portkey provider value |
| `x-portkey-provider: portkey` | 500 | Causes internal self-referential proxy loop |
| `x-portkey-provider: anthropic` without `x-portkey-api-key` | 500 | Gateway can't reach Anthropic backend without the virtual key |
| Model names without `@` prefix | 412 | Model not on allowlist; `@` prefix is required syntax |

## Orchid integration

The `portkey-aipe` profile in `~/.config/orchid/config.json` uses the `headers` map:

```json
"portkey-aipe": {
  "provider": "anthropic",
  "base_url": "https://portkey.aipe.cba",
  "model": "@bedrock-eus1/us.anthropic.claude-sonnet-4-6",
  "headers": {
    "Authorization": "env.AIPE_PORTKEY_AUTH_TOKEN",
    "x-portkey-api-key": "env.AIPE_PORTKEY_AUTH_TOKEN",
    "x-portkey-provider": "anthropic"
  }
}
```

`env.<VAR>` values are resolved at request time. `api_key` can be omitted when `Authorization` is in `headers`.

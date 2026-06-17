# Config Changes

## File: `src/client/mod.rs`

### Current (blocking line)

```rust
match provider_name {
    "anthropic" => { /* ... */ }
    _ => Err(ProviderError::InvalidResponse("unknown provider")),
}
```

### Change (5 lines)

```rust
match provider_name {
    "anthropic" => { /* existing */ }
    "openai" => {
        let mut client = openai::OpenAiClient::from_profile(profile)?;
        if let Some(path) = log_path { client = client.with_log(path); }
        Ok(Arc::new(client))
    }
    _ => Err(ProviderError::InvalidResponse(...)),
}
```

### Profile wire format (no changes needed)

Profiles already support all required fields:

```jsonc
{
  "profiles": {
    "lm-studio": {
      "provider": "openai",    // new value
      "base_url": "http://localhost:1234",  // already supported
      "model": "your-model-name",           // already supported
      "api_key": "lm-studio-key",           // already supported
      "max_tokens": 4096                    // already supported
    }
  }
}
```

## Profile struct (no changes needed)

Defined at [`src/config/mod.rs:8-30`](../src/config/mod.rs). All fields needed already exist:

- `name`, `provider`, `api_key`, `base_url`, `model`, `max_tokens`, `headers`, `env`

## CLI (no changes needed)

`config use <profile-name>` already works. Just pick a profile with `"provider": "openai"`.

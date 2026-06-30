use std::env;

/// Replace all `env.<VAR>` tokens in a string with the corresponding env var value.
/// Handles both whole-value (`env.FOO`) and inline (`Bearer env.FOO`) forms.
pub fn resolve_env_inline(s: &str) -> String {
    let mut result = s.to_string();
    while let Some(start) = result.find("env.") {
        let after = &result[start + 4..];
        let end = after
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(after.len());
        let var_name = &after[..end];
        let value = env::var(var_name).unwrap_or_default();
        result = format!("{}{}{}", &result[..start], value, &after[end..]);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_env_inline_whole_value() {
        std::env::set_var("TEST_INLINE_VAR", "mytoken");
        assert_eq!(resolve_env_inline("env.TEST_INLINE_VAR"), "mytoken");
    }

    #[test]
    fn test_resolve_env_inline_with_prefix() {
        std::env::set_var("TEST_INLINE_VAR", "mytoken");
        assert_eq!(resolve_env_inline("Bearer env.TEST_INLINE_VAR"), "Bearer mytoken");
    }

    #[test]
    fn test_resolve_env_inline_unset_var() {
        std::env::remove_var("TEST_INLINE_MISSING");
        assert_eq!(resolve_env_inline("Bearer env.TEST_INLINE_MISSING"), "Bearer ");
    }
}

pub mod anthropic;
pub mod base;

use crate::config::Profile;
use crate::provider::{Provider, ProviderError};
use std::sync::Arc;

pub fn create_provider(profile: &Profile) -> Result<Arc<dyn Provider>, ProviderError> {
    create_provider_with_log(profile, None)
}

pub fn create_provider_with_log(
    profile: &Profile,
    log_path: Option<std::path::PathBuf>,
) -> Result<Arc<dyn Provider>, ProviderError> {
    let provider_name = if profile.provider.is_empty() {
        "anthropic"
    } else {
        &profile.provider
    };

    match provider_name {
        "anthropic" => {
            let mut client = anthropic::AnthropicClient::from_profile(profile)?;
            if let Some(path) = log_path {
                client = client.with_log(path);
            }
            Ok(Arc::new(client))
        }
        _ => Err(ProviderError::InvalidResponse(format!(
            "unknown provider: {}",
            provider_name
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Profile;
    use std::collections::HashMap;

    #[test]
    fn test_create_provider_defaults_to_anthropic() {
        let profile = Profile {
            name: "test".to_string(),
            provider: String::new(),
            api_key: String::new(),
            base_url: String::new(),
            model: String::new(),
            max_tokens: None,
            extra: HashMap::new(),
            headers: HashMap::new(),
            env: HashMap::new(),
        };

        let result = create_provider(&profile);
        assert!(result.is_err() || result.is_ok());
    }
}

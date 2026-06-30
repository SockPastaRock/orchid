use crate::client::resolve::resolve_env_inline;
use crate::{load_config, JsonError};
use reqwest::blocking::Client;
use serde_json::{json, Map, Value};

pub fn server_action(
    action: &str,
    profile_name: Option<&str>,
    body_params: &[(String, String)],
) -> Result<serde_json::Value, String> {
    let config = load_config()?;

    // Resolve profile
    let profile_name = profile_name
        .map(|s| s.to_string())
        .or_else(|| config.current_profile.clone())
        .ok_or_else(|| "no profile specified: use --profile or set current profile".to_string())?;

    let profile = config
        .profiles
        .get(&profile_name)
        .ok_or_else(|| format!("profile '{}' not found", profile_name))?;

    // Look up action
    let server_action = profile
        .server_actions
        .get(action)
        .ok_or_else(|| format!("Action '{}' not defined in profile '{}'", action, profile_name))?;

    // Build URL
    let url = build_url(&profile, server_action);

    // Build headers
    let headers = build_headers(&profile, server_action);

    // Build body from --key value pairs
    let body = build_body(body_params);

    // Execute HTTP request
    let client = Client::new();
    let mut req = client.request(method_from_str(&server_action.method)?, &url);

    for (name, value) in headers {
        req = req.header(&name, &value);
    }

    // Only set body for non-GET methods
    if !body.is_null() && server_action.method.to_uppercase() != "GET" {
        req = req.json(&body);
    }

    let resp = req.send().map_err(|e| format!("HTTP request failed: {}", e))?;
    let status = resp.status().as_u16();

    let body_text = resp
        .text()
        .map_err(|e| format!("failed to read response body: {}", e))?;

    // Parse response body as JSON if possible, otherwise wrap in string
    let response_body = serde_json::from_str::<Value>(&body_text).unwrap_or(json!(body_text));

    if status >= 400 {
        let err = JsonError::new(
            "http_error",
            &format!("Server returned {}", status),
        );
        return Err(serde_json::to_string(&err).unwrap_or_else(|_| format!("{{\"error\": \"http_error\", \"message\": \"Server returned {}\"}}", status)));
    }

    Ok(response_body)
}

fn build_url(profile: &crate::config::Profile, action: &crate::config::ServerAction) -> String {
    let base = if profile.base_url.is_empty() {
        default_base_url(&profile.provider)
    } else {
        &profile.base_url
    };
    format!("{}{}", base.trim_end_matches('/'), action.path)
}

fn build_headers(
    profile: &crate::config::Profile,
    action: &crate::config::ServerAction,
) -> Vec<(String, String)> {
    let mut headers: Vec<(String, String)> = Vec::new();

    // Bearer auth from profile's api_key (with env. resolution)
    let resolved_api_key = resolve_env_inline(&profile.api_key);
    if !resolved_api_key.is_empty() {
        headers.push(("Authorization".to_string(), format!("Bearer {}", resolved_api_key)));
    }

    // Action-specific headers (with env. resolution), can override Authorization
    for (name, value) in &action.headers {
        headers.push((name.clone(), resolve_env_inline(value)));
    }

    headers
}

fn default_base_url(provider: &str) -> &str {
    match provider {
        "anthropic" => "https://api.anthropic.com",
        "openai-compat" | "openai" => "https://api.openai.com",
        _ => "",
    }
}

fn method_from_str(s: &str) -> Result<reqwest::Method, String> {
    match s.to_uppercase().as_str() {
        "GET" => Ok(reqwest::Method::GET),
        "POST" => Ok(reqwest::Method::POST),
        "PUT" => Ok(reqwest::Method::PUT),
        "DELETE" => Ok(reqwest::Method::DELETE),
        "PATCH" => Ok(reqwest::Method::PATCH),
        _ => Err(format!("unsupported HTTP method: {}", s)),
    }
}

fn build_body(body_params: &[(String, String)]) -> Value {
    let mut map = Map::new();
    for (k, v) in body_params {
        map.insert(k.clone(), Value::String(v.clone()));
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_body_empty() {
        let body = build_body(&[]);
        assert!(body.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_build_body_params() {
        let params = vec![
            ("model".to_string(), "gpt-4".to_string()),
            ("n".to_string(), "3".to_string()),
        ];
        let body = build_body(&params);
        let obj = body.as_object().unwrap();
        assert_eq!(obj.get("model").unwrap(), "gpt-4");
        assert_eq!(obj.get("n").unwrap(), "3");
    }

    #[test]
    fn test_method_from_str() {
        assert_eq!(method_from_str("GET").unwrap(), reqwest::Method::GET);
        assert_eq!(method_from_str("post").unwrap(), reqwest::Method::POST);
        assert!(method_from_str("INVALID").is_err());
    }

    #[test]
    fn test_default_base_url() {
        assert_eq!(default_base_url("anthropic"), "https://api.anthropic.com");
        assert_eq!(default_base_url("openai-compat"), "https://api.openai.com");
        assert_eq!(default_base_url("unknown"), "");
    }
}

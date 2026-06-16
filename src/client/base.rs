use crate::provider::ProviderError;
use reqwest::blocking::{Client, Response};
use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const READ_TIMEOUT: Duration = Duration::from_secs(600);
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 100;

pub struct BaseClient {
    client: Client,
    /// Optional path for debug logging (mirrors DiagLogger but avoids a circular dep).
    log_path: Option<std::path::PathBuf>,
}

impl BaseClient {
    pub fn new() -> Result<Self, ProviderError> {
        let client = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(READ_TIMEOUT)
            .gzip(true)
            .user_agent("orchid/1.0")
            .build()
            .map_err(|e| ProviderError::Network(format!("failed to create client: {}", e)))?;

        Ok(BaseClient { client, log_path: None })
    }

    pub fn with_log(mut self, path: std::path::PathBuf) -> Self {
        self.log_path = Some(path);
        self
    }

    pub(crate) fn log_debug(&self, event: &str, detail: &str) {
        let Some(ref path) = self.log_path else { return };
        let entry = serde_json::json!({
            "ts": chrono::Utc::now(),
            "level": "DEBUG",
            "event": event,
            "detail": detail,
        });
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = std::io::Write::write_fmt(&mut f, format_args!("{}", entry));
            let _ = std::io::Write::write_fmt(&mut f, format_args!("\n"));
        }
    }

    /// Fire a POST and return the raw streaming `Response` for SSE consumption.
    /// No retry — SSE connections are not safely retriable mid-stream.
    pub fn post_streaming(
        &self,
        url: &str,
        body: String,
        headers: &[(&str, &str)],
    ) -> Result<Response, ProviderError> {
        let header_names = headers.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(", ");
        self.log_debug("http_request", &format!("POST {} headers=[{}] body_len={}", url, header_names, body.len()));

        let mut req = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .body(body);

        for (k, v) in headers {
            req = req.header(*k, *v);
        }

        let response = req
            .send()
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let status = response.status().as_u16();
        self.log_debug("http_response", &format!("status={}", status));

        if !response.status().is_success() {
            if status == 401 {
                let body = response.text().unwrap_or_default();
                self.log_debug("http_error_body", &body);
                return Err(ProviderError::AuthError("invalid API key".to_string()));
            }
            let body = response.text().unwrap_or_default();
            self.log_debug("http_error_body", &body);
            return Err(ProviderError::InvalidResponse(format!("HTTP {}: {}", status, body)));
        }

        Ok(response)
    }

    pub fn post_with_retry(
        &self,
        url: &str,
        body: String,
        headers: &[(&str, &str)],
    ) -> Result<String, ProviderError> {
        let header_names = headers.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(", ");
        self.log_debug("http_request", &format!("POST {} headers=[{}] body_len={}", url, header_names, body.len()));

        let mut attempt = 0;

        loop {
            let mut req = self
                .client
                .post(url)
                .header("Content-Type", "application/json")
                .body(body.clone());

            for (k, v) in headers {
                req = req.header(*k, *v);
            }

            let response = req.send();

            match response {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    self.log_debug("http_response", &format!("status={} attempt={}", status, attempt));

                    if resp.status().is_success() {
                        return resp.text().map_err(|e| {
                            ProviderError::Network(format!("failed to read body: {}", e))
                        });
                    }

                    if is_retryable(status) && attempt < MAX_RETRIES {
                        attempt += 1;
                        let delay_ms = RETRY_DELAY_MS * 2_u64.pow(attempt - 1);
                        std::thread::sleep(Duration::from_millis(delay_ms));
                        continue;
                    }

                    let body = resp.text().unwrap_or_default();
                    self.log_debug("http_error_body", &body);

                    if status == 401 {
                        return Err(ProviderError::AuthError("invalid API key".to_string()));
                    }

                    return Err(ProviderError::InvalidResponse(format!("HTTP {}: {}", status, body)));
                }
                Err(e) => {
                    self.log_debug("http_send_error", &e.to_string());
                    if attempt < MAX_RETRIES && e.is_timeout() {
                        attempt += 1;
                        let delay_ms = RETRY_DELAY_MS * 2_u64.pow(attempt - 1);
                        std::thread::sleep(Duration::from_millis(delay_ms));
                        continue;
                    }

                    return Err(ProviderError::Network(e.to_string()));
                }
            }
        }
    }
}

pub fn is_retryable(status: u16) -> bool {
    matches!(status, 408 | 429 | 500 | 502 | 503 | 504)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_retryable() {
        assert!(is_retryable(408));
        assert!(is_retryable(429));
        assert!(is_retryable(500));
        assert!(is_retryable(502));
        assert!(is_retryable(503));
        assert!(is_retryable(504));
        assert!(!is_retryable(400));
        assert!(!is_retryable(401));
    }

    #[test]
    fn test_base_client_creation() {
        let client = BaseClient::new();
        assert!(client.is_ok());
    }
}

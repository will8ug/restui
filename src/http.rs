use crate::parser::Method;
use crate::vars::ResolvedRequest;
use reqwest::header::CONTENT_TYPE;
use std::fmt;
use std::time::Duration;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq)]
pub struct AppResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub content_type: Option<String>,
    pub duration: Duration,
    pub size_bytes: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HttpError {
    pub message: String,
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HTTP error: {}", self.message)
    }
}

pub fn send_request(
    client: &reqwest::blocking::Client,
    request: &ResolvedRequest,
) -> Result<AppResponse, HttpError> {
    let method = match request.method {
        Method::Get => reqwest::Method::GET,
        Method::Post => reqwest::Method::POST,
        Method::Put => reqwest::Method::PUT,
        Method::Delete => reqwest::Method::DELETE,
        Method::Patch => reqwest::Method::PATCH,
        Method::Head => reqwest::Method::HEAD,
        Method::Options => reqwest::Method::OPTIONS,
    };

    let mut builder = client.request(method, &request.url);

    for (name, value) in &request.headers {
        builder = builder.header(name, value);
    }

    if let Some(body) = &request.body {
        builder = builder.body(body.clone());
    }

    let started_at = Instant::now();
    let response = builder.send().map_err(|error| HttpError {
        message: format!("request failed: {error}"),
    })?;
    let duration = started_at.elapsed();

    let status = response.status();
    let status_text = status.canonical_reason().unwrap_or_default().to_string();
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| {
            (
                name.to_string(),
                value.to_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = response.text().map_err(|error| HttpError {
        message: format!("failed to read response body: {error}"),
    })?;
    let size_bytes = body.len();

    Ok(AppResponse {
        status: status.as_u16(),
        status_text,
        headers,
        body,
        content_type,
        duration,
        size_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::blocking::Client;
    use wiremock::matchers::{body_string, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn resolved_request(
        method: Method,
        url: String,
        headers: Vec<(String, String)>,
        body: Option<String>,
    ) -> ResolvedRequest {
        ResolvedRequest {
            method,
            url,
            headers,
            body,
        }
    }

    async fn send_via_blocking_client(request: ResolvedRequest) -> Result<AppResponse, HttpError> {
        tokio::task::spawn_blocking(move || {
            let client = Client::new();
            send_request(&client, &request)
        })
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn send_get_200() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/users"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("hello world")
                    .insert_header("x-test", "ok")
                    .insert_header("content-type", "text/plain"),
            )
            .mount(&server)
            .await;

        let response = send_via_blocking_client(resolved_request(
            Method::Get,
            format!("{}/users", server.uri()),
            vec![],
            None,
        ))
        .await
        .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.status_text, "OK");
        assert_eq!(response.body, "hello world");
        assert_eq!(response.content_type.as_deref(), Some("text/plain"));
        assert!(response.headers.iter().any(|(name, value)| name == "x-test" && value == "ok"));
    }

    #[tokio::test]
    async fn send_post_with_json_body() {
        let server = MockServer::start().await;
        let json = r#"{"name":"restui"}"#;

        Mock::given(method("POST"))
            .and(path("/items"))
            .and(header("content-type", "application/json"))
            .and(body_string(json))
            .respond_with(ResponseTemplate::new(201).set_body_string("created"))
            .mount(&server)
            .await;

        let response = send_via_blocking_client(resolved_request(
            Method::Post,
            format!("{}/items", server.uri()),
            vec![("content-type".to_string(), "application/json".to_string())],
            Some(json.to_string()),
        ))
        .await
        .unwrap();

        assert_eq!(response.status, 201);
        assert_eq!(response.body, "created");
    }

    #[tokio::test]
    async fn send_put_request() {
        let server = MockServer::start().await;

        Mock::given(method("PUT"))
            .and(path("/items/1"))
            .respond_with(ResponseTemplate::new(200).set_body_string("updated"))
            .mount(&server)
            .await;

        let response = send_via_blocking_client(resolved_request(
            Method::Put,
            format!("{}/items/1", server.uri()),
            vec![],
            Some("payload".to_string()),
        ))
        .await
        .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body, "updated");
    }

    #[tokio::test]
    async fn send_delete_request() {
        let server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path("/items/1"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let response = send_via_blocking_client(resolved_request(
            Method::Delete,
            format!("{}/items/1", server.uri()),
            vec![],
            None,
        ))
        .await
        .unwrap();

        assert_eq!(response.status, 204);
        assert_eq!(response.body, "");
    }

    #[tokio::test]
    async fn send_with_custom_headers() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/headers"))
            .and(header("x-api-key", "secret"))
            .and(header("x-trace-id", "trace-123"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;

        let response = send_via_blocking_client(resolved_request(
            Method::Get,
            format!("{}/headers", server.uri()),
            vec![
                ("x-api-key".to_string(), "secret".to_string()),
                ("x-trace-id".to_string(), "trace-123".to_string()),
            ],
            None,
        ))
        .await
        .unwrap();

        assert_eq!(response.status, 200);
    }

    #[tokio::test]
    async fn send_response_headers_captured() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/response-headers"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("x-response-id", "abc123")
                    .insert_header("cache-control", "no-cache"),
            )
            .mount(&server)
            .await;

        let response = send_via_blocking_client(resolved_request(
            Method::Get,
            format!("{}/response-headers", server.uri()),
            vec![],
            None,
        ))
        .await
        .unwrap();

        assert!(response
            .headers
            .iter()
            .any(|(name, value)| name == "x-response-id" && value == "abc123"));
        assert!(response
            .headers
            .iter()
            .any(|(name, value)| name == "cache-control" && value == "no-cache"));
    }

    #[tokio::test]
    async fn send_measures_duration() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/slow"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(std::time::Duration::from_millis(20))
                    .set_body_string("slow"),
            )
            .mount(&server)
            .await;

        let response = send_via_blocking_client(resolved_request(
            Method::Get,
            format!("{}/slow", server.uri()),
            vec![],
            None,
        ))
        .await
        .unwrap();

        assert!(response.duration > Duration::ZERO);
    }

    #[tokio::test]
    async fn send_404_response() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/missing"))
            .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
            .mount(&server)
            .await;

        let response = send_via_blocking_client(resolved_request(
            Method::Get,
            format!("{}/missing", server.uri()),
            vec![],
            None,
        ))
        .await
        .unwrap();

        assert_eq!(response.status, 404);
        assert_eq!(response.status_text, "Not Found");
        assert_eq!(response.body, "not found");
    }

    #[tokio::test]
    async fn send_connection_error() {
        let error = send_via_blocking_client(resolved_request(
            Method::Get,
            "not a valid url".to_string(),
            vec![],
            None,
        ))
        .await
        .unwrap_err();

        assert!(error.message.contains("request failed"));
    }

    #[tokio::test]
    async fn send_large_response_body() {
        let server = MockServer::start().await;
        let body = "x".repeat(4096);

        Mock::given(method("GET"))
            .and(path("/large"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body.clone()))
            .mount(&server)
            .await;

        let response = send_via_blocking_client(resolved_request(
            Method::Get,
            format!("{}/large", server.uri()),
            vec![],
            None,
        ))
        .await
        .unwrap();

        assert_eq!(response.body, body);
        assert_eq!(response.size_bytes, 4096);
    }

    #[tokio::test]
    async fn send_content_type_extracted() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/json"))
            .respond_with(ResponseTemplate::new(200).set_body_raw("{}", "application/json"))
            .mount(&server)
            .await;

        let response = send_via_blocking_client(resolved_request(
            Method::Get,
            format!("{}/json", server.uri()),
            vec![],
            None,
        ))
        .await
        .unwrap();

        assert_eq!(response.content_type.as_deref(), Some("application/json"));
    }
}

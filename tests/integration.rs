// Integration tests for the full parse → resolve → send pipeline

use reqwest::blocking::Client;
use restui::{http, parser, vars};
use wiremock::matchers::{body_string, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn send_via_blocking_client(
    request: vars::ResolvedRequest,
) -> Result<http::AppResponse, http::HttpError> {
    tokio::task::spawn_blocking(move || {
        let client = Client::new();
        http::send_request(&client, &request)
    })
    .await
    .expect("blocking task should finish")
}

#[tokio::test]
async fn test_full_flow_parse_resolve_send() {
    let server = MockServer::start().await;
    let body = r#"{"name":"restui"}"#;

    Mock::given(method("POST"))
        .and(path("/users"))
        .and(header("content-type", "application/json"))
        .and(body_string(body))
        .respond_with(
            ResponseTemplate::new(201)
                .insert_header("content-type", "application/json")
                .set_body_raw(r#"{"created":true}"#, "application/json"),
        )
        .mount(&server)
        .await;

    let input = format!(
        "@host = {}\n@content_type = application/json\n\n### Create user\nPOST {{{{host}}}}/users\nContent-Type: {{{{content_type}}}}\n\n{}",
        server.uri(), body
    );

    let parsed = parser::parse(&input).expect("request file should parse");
    let resolved =
        vars::resolve(&parsed.variables, &parsed.requests[0]).expect("variables should resolve");
    let response = send_via_blocking_client(resolved)
        .await
        .expect("request should succeed");

    assert_eq!(parsed.requests.len(), 1);
    assert_eq!(response.status, 201);
    assert_eq!(response.status_text, "Created");
    assert_eq!(response.body, r#"{"created":true}"#);
    assert_eq!(response.content_type.as_deref(), Some("application/json"));
    assert!(response.size_bytes > 0);
}

#[test]
fn test_multiple_requests_parsed_and_resolved() {
    let input = "@host = https://api.example.com\n@token = secret-token\n\n### List users\nGET {{host}}/users\nAccept: application/json\n\n### Update user\nPATCH {{host}}/users/42\nAuthorization: Bearer {{token}}\nContent-Type: application/json\n\n{\n  \"role\": \"admin\"\n}";

    let parsed = parser::parse(input).expect("request file should parse");

    assert_eq!(parsed.requests.len(), 2);

    let first = vars::resolve(&parsed.variables, &parsed.requests[0])
        .expect("first request should resolve");
    let second = vars::resolve(&parsed.variables, &parsed.requests[1])
        .expect("second request should resolve");

    assert_eq!(first.method, parser::Method::Get);
    assert_eq!(first.url, "https://api.example.com/users");
    assert_eq!(
        first.headers,
        vec![("Accept".to_string(), "application/json".to_string())]
    );
    assert_eq!(first.body, None);

    assert_eq!(second.method, parser::Method::Patch);
    assert_eq!(second.url, "https://api.example.com/users/42");
    assert_eq!(
        second.headers,
        vec![
            (
                "Authorization".to_string(),
                "Bearer secret-token".to_string()
            ),
            ("Content-Type".to_string(), "application/json".to_string()),
        ]
    );
    assert_eq!(second.body.as_deref(), Some("{\n  \"role\": \"admin\"\n}"));
}

#[test]
fn test_undefined_variable_error_propagation() {
    let input = "GET {{host}}/users\nAuthorization: Bearer {{undefined}}";
    let parsed = parser::parse(input).expect("request file should parse");
    let error = vars::resolve(&parsed.variables, &parsed.requests[0])
        .expect_err("undefined variable should fail resolution");

    assert_eq!(error.variable_name, "host");
    assert_eq!(error.field, "url");
}

#[test]
fn test_parse_error_on_invalid_method() {
    let error = parser::parse("TRACE https://example.com")
        .expect_err("invalid method should return parse error");

    assert_eq!(error.line, 1);
    assert!(error.message.contains("Unsupported HTTP method 'TRACE'"));
}

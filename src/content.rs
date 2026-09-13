use serde_json::Value;
use unicode_width::UnicodeWidthStr;

use crate::http::AppResponse;
use crate::parser::{Method, ParsedRequest, Variable};
use crate::vars;

pub fn format_response(response: &AppResponse) -> String {
    let mut lines = vec![format!("HTTP {} {}", response.status, response.status_text)];

    for (name, value) in &response.headers {
        lines.push(format!("{name}: {value}"));
    }

    lines.push(String::new());
    lines.push(format_body(response));
    lines.join("\n")
}

pub fn format_body(response: &AppResponse) -> String {
    let is_json = response
        .content_type
        .as_deref()
        .map(|content_type| content_type.to_ascii_lowercase().contains("json"))
        .unwrap_or(false);

    if !is_json {
        return response.body.clone();
    }

    match serde_json::from_str::<Value>(&response.body)
        .ok()
        .and_then(|value| serde_json::to_string_pretty(&value).ok())
    {
        Some(pretty) => pretty,
        None => response.body.clone(),
    }
}

pub fn format_request(request: &ParsedRequest) -> String {
    format_parts(
        &request.method,
        &request.url,
        &request.headers,
        request.body.as_deref(),
    )
}

pub fn format_request_detail(variables: &[Variable], request: &ParsedRequest) -> String {
    match vars::resolve(variables, request) {
        Ok(resolved) => format_parts(
            &resolved.method,
            &resolved.url,
            &resolved.headers,
            resolved.body.as_deref(),
        ),
        Err(_) => format_request(request),
    }
}

fn format_parts(
    method: &Method,
    url: &str,
    headers: &[(String, String)],
    body: Option<&str>,
) -> String {
    let mut lines = vec![format!("{method} {url}")];

    if !headers.is_empty() {
        lines.push(String::new());
        for (name, value) in headers {
            lines.push(format!("{name}: {value}"));
        }
    }

    if let Some(body) = body {
        lines.push(String::new());
        lines.push(body.to_string());
    }

    lines.join("\n")
}

pub fn request_label(request: &ParsedRequest) -> String {
    request
        .name
        .clone()
        .unwrap_or_else(|| format!("{} {}", request.method, url_display(&request.url)))
}

pub fn url_display(url: &str) -> String {
    if let Some(path) = http_url_path(url) {
        path.to_string()
    } else {
        url.to_string()
    }
}

pub fn http_url_path(url: &str) -> Option<&str> {
    let scheme_index = url.find("://")?;
    let path_start = url[scheme_index + 3..].find('/')? + scheme_index + 3;
    Some(&url[path_start..])
}

/// Display-line lower bound: word-wrap only produces more lines than width division.
pub fn wrapped_line_count(text: &str, width: usize) -> usize {
    if width == 0 {
        return text.lines().count().max(1);
    }
    text.lines()
        .map(|line| UnicodeWidthStr::width(line).div_ceil(width).max(1))
        .sum()
}

pub fn max_line_width(text: &str) -> usize {
    text.lines().map(UnicodeWidthStr::width).max().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{Method, ParsedRequest};

    #[test]
    fn test_no_extra_blank_lines_without_headers_or_body() {
        let req = ParsedRequest {
            name: Some("Bare".to_string()),
            method: Method::Get,
            url: "https://example.com".to_string(),
            headers: vec![],
            body: None,
            source_line: 1,
        };

        let text = format_request(&req);

        assert_eq!(text, "GET https://example.com");
    }

    #[test]
    fn test_format_request_detail_resolves_variables() {
        let req = ParsedRequest {
            name: Some("Get".to_string()),
            method: Method::Get,
            url: "{{host}}/get".to_string(),
            headers: vec![
                ("Accept".to_string(), "{{content_type}}".to_string()),
                ("Authorization".to_string(), "Bearer {{token}}".to_string()),
            ],
            body: Some("{\"user\": \"{{username}}\"}".to_string()),
            source_line: 1,
        };
        let variables = vec![
            Variable {
                name: "host".to_string(),
                value: "https://httpbin.org".to_string(),
            },
            Variable {
                name: "content_type".to_string(),
                value: "application/json".to_string(),
            },
            Variable {
                name: "token".to_string(),
                value: "abc123".to_string(),
            },
            Variable {
                name: "username".to_string(),
                value: "alice".to_string(),
            },
        ];

        let text = format_request_detail(&variables, &req);

        assert_eq!(
            text,
            "GET https://httpbin.org/get\n\nAccept: application/json\nAuthorization: Bearer abc123\n\n{\"user\": \"alice\"}"
        );
    }

    #[test]
    fn test_format_request_detail_resolves_transitive_variables() {
        let req = ParsedRequest {
            name: Some("Get".to_string()),
            method: Method::Get,
            url: "{{origin}}/ping".to_string(),
            headers: vec![],
            body: None,
            source_line: 1,
        };
        let variables = vec![
            Variable {
                name: "host".to_string(),
                value: "httpbin.org".to_string(),
            },
            Variable {
                name: "origin".to_string(),
                value: "https://{{host}}".to_string(),
            },
        ];

        let text = format_request_detail(&variables, &req);

        assert_eq!(text, "GET https://httpbin.org/ping");
    }

    #[test]
    fn test_format_request_detail_falls_back_to_raw_on_undefined_variable() {
        let req = ParsedRequest {
            name: Some("Get".to_string()),
            method: Method::Get,
            url: "{{missing}}/get".to_string(),
            headers: vec![],
            body: None,
            source_line: 1,
        };

        let text = format_request_detail(&[], &req);

        assert_eq!(text, "GET {{missing}}/get");
    }

    #[test]
    fn test_format_request_detail_falls_back_to_raw_on_circular_variable() {
        let req = ParsedRequest {
            name: Some("Get".to_string()),
            method: Method::Get,
            url: "{{a}}/get".to_string(),
            headers: vec![],
            body: None,
            source_line: 1,
        };
        let variables = vec![
            Variable {
                name: "a".to_string(),
                value: "{{b}}".to_string(),
            },
            Variable {
                name: "b".to_string(),
                value: "{{a}}".to_string(),
            },
        ];

        let text = format_request_detail(&variables, &req);

        assert_eq!(text, "GET {{a}}/get");
    }

    #[test]
    fn test_max_line_width_takes_maximum_across_lines() {
        assert_eq!(max_line_width("abc\ndefgh\nxy"), 5);
    }

    #[test]
    fn test_max_line_width_of_empty_text_is_zero() {
        assert_eq!(max_line_width(""), 0);
    }

    #[test]
    fn test_max_line_width_of_single_line_is_its_width() {
        assert_eq!(max_line_width("hello"), 5);
    }

    #[test]
    fn test_request_label_prefers_name() {
        let req = ParsedRequest {
            name: Some("Named".to_string()),
            method: Method::Get,
            url: "https://example.com/users".to_string(),
            headers: vec![],
            body: None,
            source_line: 1,
        };

        assert_eq!(request_label(&req), "Named");
    }

    #[test]
    fn test_request_label_falls_back_to_method_and_path() {
        let req = ParsedRequest {
            name: None,
            method: Method::Get,
            url: "https://example.com/users".to_string(),
            headers: vec![],
            body: None,
            source_line: 1,
        };

        assert_eq!(request_label(&req), "GET /users");
    }
}

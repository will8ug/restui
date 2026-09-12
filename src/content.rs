use serde_json::Value;
use unicode_width::UnicodeWidthStr;

use crate::http::AppResponse;
use crate::parser::ParsedRequest;

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
    let mut lines = vec![format!("{} {}", request.method, request.url)];

    if !request.headers.is_empty() {
        lines.push(String::new());
        for (name, value) in &request.headers {
            lines.push(format!("{name}: {value}"));
        }
    }

    if let Some(body) = &request.body {
        lines.push(String::new());
        lines.push(body.clone());
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

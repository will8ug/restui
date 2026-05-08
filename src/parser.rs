use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Method::Get => write!(f, "GET"),
            Method::Post => write!(f, "POST"),
            Method::Put => write!(f, "PUT"),
            Method::Delete => write!(f, "DELETE"),
            Method::Patch => write!(f, "PATCH"),
            Method::Head => write!(f, "HEAD"),
            Method::Options => write!(f, "OPTIONS"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variable {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedRequest {
    pub name: Option<String>,
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    pub source_line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFile {
    pub variables: Vec<Variable>,
    pub requests: Vec<ParsedRequest>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Parse error at line {}: {}", self.line, self.message)
    }
}

pub fn parse(input: &str) -> Result<ParsedFile, ParseError> {
    let lines: Vec<(usize, &str)> = input
        .lines()
        .enumerate()
        .map(|(index, line)| (index + 1, line))
        .collect();
    let mut variables = Vec::new();
    let mut requests = Vec::new();
    let mut index = 0;
    let mut pending_name = None;

    while index < lines.len() {
        let (line_number, line) = lines[index];

        if line.trim().is_empty() || is_comment_line(line) {
            index += 1;
            continue;
        }

        if let Some(name) = parse_delimiter_line(line) {
            pending_name = name;
            index += 1;
            continue;
        }

        if is_variable_line(line) {
            variables.push(parse_variable_line(line, line_number)?);
            index += 1;
            continue;
        }

        let (request, next_index) = parse_request(&lines, index, pending_name.take())?;
        requests.push(request);
        index = next_index;
    }

    Ok(ParsedFile {
        variables,
        requests,
    })
}

fn parse_request(
    lines: &[(usize, &str)],
    start_index: usize,
    name: Option<String>,
) -> Result<(ParsedRequest, usize), ParseError> {
    let (source_line, request_line) = lines[start_index];
    let (method, mut url) = parse_request_line(request_line, source_line)?;
    let mut headers = Vec::new();
    let mut body_lines = Vec::new();
    let mut index = start_index + 1;
    let mut headers_started = false;
    let mut body_started = false;

    while index < lines.len() {
        let (line_number, line) = lines[index];

        if parse_delimiter_line(line).is_some() {
            break;
        }

        if body_started {
            body_lines.push(line.to_string());
            index += 1;
            continue;
        }

        if line.trim().is_empty() {
            body_started = true;
            index += 1;
            continue;
        }

        if is_comment_line(line) {
            index += 1;
            continue;
        }

        if !headers_started {
            if let Some(query_fragment) = parse_query_line(line) {
                url.push_str(&query_fragment);
                index += 1;
                continue;
            }
        }

        headers_started = true;
        headers.push(parse_header_line(line, line_number)?);
        index += 1;
    }

    let body = if body_started && !body_lines.is_empty() {
        Some(body_lines.join("\n"))
    } else {
        None
    };

    Ok((
        ParsedRequest {
            name,
            method,
            url,
            headers,
            body,
            source_line,
        },
        index,
    ))
}

fn parse_request_line(line: &str, line_number: usize) -> Result<(Method, String), ParseError> {
    let trimmed = line.trim();
    let mut parts = trimmed.split_whitespace();
    let first = parts.next().ok_or_else(|| ParseError {
        message: "Missing request line".to_string(),
        line: line_number,
    })?;

    if first.starts_with("http://") || first.starts_with("https://") {
        let url = first.to_string();

        if let Some(version) = parts.next() {
            if !version.starts_with("HTTP/") || parts.next().is_some() {
                return Err(ParseError {
                    message: "Invalid request line".to_string(),
                    line: line_number,
                });
            }
        }

        return Ok((Method::Get, url));
    }

    let method = parse_method(first).ok_or_else(|| ParseError {
        message: format!("Unsupported HTTP method '{first}'"),
        line: line_number,
    })?;
    let url = parts.next().ok_or_else(|| ParseError {
        message: "Missing request URL".to_string(),
        line: line_number,
    })?;

    if let Some(version) = parts.next() {
        if !version.starts_with("HTTP/") || parts.next().is_some() {
            return Err(ParseError {
                message: "Invalid request line".to_string(),
                line: line_number,
            });
        }
    }

    Ok((method, url.to_string()))
}

fn parse_method(input: &str) -> Option<Method> {
    match input.to_ascii_uppercase().as_str() {
        "GET" => Some(Method::Get),
        "POST" => Some(Method::Post),
        "PUT" => Some(Method::Put),
        "DELETE" => Some(Method::Delete),
        "PATCH" => Some(Method::Patch),
        "HEAD" => Some(Method::Head),
        "OPTIONS" => Some(Method::Options),
        _ => None,
    }
}

fn parse_header_line(line: &str, line_number: usize) -> Result<(String, String), ParseError> {
    let (name, value) = line.split_once(':').ok_or_else(|| ParseError {
        message: "Invalid header format".to_string(),
        line: line_number,
    })?;
    let name = name.trim();

    if name.is_empty() {
        return Err(ParseError {
            message: "Header name cannot be empty".to_string(),
            line: line_number,
        });
    }

    Ok((name.to_string(), value.trim().to_string()))
}

fn is_variable_line(line: &str) -> bool {
    line.trim_start().starts_with('@')
}

fn parse_variable_line(line: &str, line_number: usize) -> Result<Variable, ParseError> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix('@').ok_or_else(|| ParseError {
        message: "Invalid variable definition".to_string(),
        line: line_number,
    })?;
    let (name, value) = rest.split_once('=').ok_or_else(|| ParseError {
        message: "Invalid variable definition".to_string(),
        line: line_number,
    })?;
    let name = name.trim();

    if name.is_empty() {
        return Err(ParseError {
            message: "Variable name cannot be empty".to_string(),
            line: line_number,
        });
    }

    Ok(Variable {
        name: name.to_string(),
        value: value.trim().to_string(),
    })
}

fn parse_delimiter_line(line: &str) -> Option<Option<String>> {
    let trimmed = line.trim_start();
    let marker_length = trimmed
        .chars()
        .take_while(|character| *character == '#')
        .count();

    if marker_length < 3 {
        return None;
    }

    let name = trimmed[marker_length..].trim();
    Some(if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    })
}

fn is_comment_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("//") || (trimmed.starts_with('#') && parse_delimiter_line(line).is_none())
}

fn parse_query_line(line: &str) -> Option<String> {
    let trimmed = line.trim();

    if trimmed.starts_with('?') || trimmed.starts_with('&') {
        Some(trimmed.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(input: &str) -> ParsedFile {
        parse(input).unwrap()
    }

    #[test]
    fn parse_simple_get() {
        let parsed = parse_ok("GET https://example.com/users");

        assert_eq!(parsed.variables, Vec::<Variable>::new());
        assert_eq!(parsed.requests.len(), 1);
        assert_eq!(
            parsed.requests[0],
            ParsedRequest {
                name: None,
                method: Method::Get,
                url: "https://example.com/users".to_string(),
                headers: vec![],
                body: None,
                source_line: 1,
            }
        );
    }

    #[test]
    fn parse_post_with_body() {
        let input = "POST https://example.com/users\nContent-Type: application/json\n\n{\"name\":\"alice\"}";
        let parsed = parse_ok(input);

        assert_eq!(parsed.requests.len(), 1);
        assert_eq!(parsed.requests[0].method, Method::Post);
        assert_eq!(
            parsed.requests[0].headers,
            vec![("Content-Type".to_string(), "application/json".to_string())]
        );
        assert_eq!(
            parsed.requests[0].body,
            Some("{\"name\":\"alice\"}".to_string())
        );
    }

    #[test]
    fn parse_multiple_requests() {
        let input = "### List users\nGET https://example.com/users\n### Create user\nPOST https://example.com/users\nContent-Type: application/json\n\n{\"name\":\"alice\"}";
        let parsed = parse_ok(input);

        assert_eq!(parsed.requests.len(), 2);
        assert_eq!(parsed.requests[0].name.as_deref(), Some("List users"));
        assert_eq!(parsed.requests[1].name.as_deref(), Some("Create user"));
        assert_eq!(parsed.requests[0].method, Method::Get);
        assert_eq!(parsed.requests[1].method, Method::Post);
    }

    #[test]
    fn parse_variables() {
        let input = "@host = https://example.com\n@token = secret\n\nGET {{host}}/users";
        let parsed = parse_ok(input);

        assert_eq!(
            parsed.variables,
            vec![
                Variable {
                    name: "host".to_string(),
                    value: "https://example.com".to_string(),
                },
                Variable {
                    name: "token".to_string(),
                    value: "secret".to_string(),
                }
            ]
        );
        assert_eq!(parsed.requests.len(), 1);
    }

    #[test]
    fn parse_comments_ignored() {
        let input = "# file comment\n// another comment\n### Named request\n# request comment\nGET https://example.com\n// header comment\nAccept: application/json";
        let parsed = parse_ok(input);

        assert_eq!(parsed.requests.len(), 1);
        assert_eq!(parsed.requests[0].name.as_deref(), Some("Named request"));
        assert_eq!(
            parsed.requests[0].headers,
            vec![("Accept".to_string(), "application/json".to_string())]
        );
    }

    #[test]
    fn parse_method_defaults_to_get() {
        let parsed = parse_ok("https://example.com/users");

        assert_eq!(parsed.requests[0].method, Method::Get);
        assert_eq!(parsed.requests[0].url, "https://example.com/users");
    }

    #[test]
    fn parse_multiline_query_params() {
        let input =
            "GET https://example.com/users\n  ?page=1\n  &limit=20\nAccept: application/json";
        let parsed = parse_ok(input);

        assert_eq!(
            parsed.requests[0].url,
            "https://example.com/users?page=1&limit=20"
        );
        assert_eq!(
            parsed.requests[0].headers,
            vec![("Accept".to_string(), "application/json".to_string())]
        );
    }

    #[test]
    fn parse_request_with_http_version() {
        let parsed = parse_ok("GET /path HTTP/1.1");

        assert_eq!(parsed.requests[0].method, Method::Get);
        assert_eq!(parsed.requests[0].url, "/path");
    }

    #[test]
    fn parse_empty_file() {
        let parsed = parse_ok("");

        assert_eq!(parsed.variables, Vec::<Variable>::new());
        assert_eq!(parsed.requests, Vec::<ParsedRequest>::new());
    }

    #[test]
    fn parse_only_variables() {
        let parsed = parse_ok("@host = https://example.com\n@token = secret");

        assert_eq!(parsed.variables.len(), 2);
        assert!(parsed.requests.is_empty());
    }

    #[test]
    fn parse_request_without_name() {
        let parsed =
            parse_ok("GET https://example.com/users\n### Named\nGET https://example.com/admin");

        assert_eq!(parsed.requests.len(), 2);
        assert_eq!(parsed.requests[0].name, None);
        assert_eq!(parsed.requests[1].name.as_deref(), Some("Named"));
    }

    #[test]
    fn parse_all_methods() {
        let input = "GET https://example.com/get\n### post\nPOST https://example.com/post\n### put\nPUT https://example.com/put\n### delete\nDELETE https://example.com/delete\n### patch\nPATCH https://example.com/patch\n### head\nHEAD https://example.com/head\n### options\nOPTIONS https://example.com/options";
        let parsed = parse_ok(input);

        assert_eq!(
            parsed
                .requests
                .iter()
                .map(|request| request.method.clone())
                .collect::<Vec<_>>(),
            vec![
                Method::Get,
                Method::Post,
                Method::Put,
                Method::Delete,
                Method::Patch,
                Method::Head,
                Method::Options,
            ]
        );
    }

    #[test]
    fn parse_headers() {
        let input =
            "GET https://example.com\nAuthorization: Bearer token\nAccept: application/json";
        let parsed = parse_ok(input);

        assert_eq!(
            parsed.requests[0].headers,
            vec![
                ("Authorization".to_string(), "Bearer token".to_string()),
                ("Accept".to_string(), "application/json".to_string()),
            ]
        );
    }

    #[test]
    fn parse_body_preserves_whitespace() {
        let input = "POST https://example.com\nContent-Type: text/plain\n\n  first line\n    second line\n\n  third line";
        let parsed = parse_ok(input);

        assert_eq!(
            parsed.requests[0].body,
            Some("  first line\n    second line\n\n  third line".to_string())
        );
    }

    #[test]
    fn parse_variables_in_url_not_resolved() {
        let parsed = parse_ok("GET {{host}}/users");

        assert_eq!(parsed.requests[0].url, "{{host}}/users");
    }

    #[test]
    fn parse_invalid_method_errors() {
        let error = parse("TRACE https://example.com").unwrap_err();

        assert_eq!(error.line, 1);
        assert!(error.message.contains("Unsupported HTTP method"));
    }

    #[test]
    fn parse_invalid_header_errors() {
        let error = parse("GET https://example.com\nAuthorization Bearer token").unwrap_err();

        assert_eq!(error.line, 2);
        assert_eq!(error.message, "Invalid header format");
    }

    #[test]
    fn parse_invalid_variable_errors() {
        let error = parse("@host https://example.com").unwrap_err();

        assert_eq!(error.line, 1);
        assert_eq!(error.message, "Invalid variable definition");
    }

    #[test]
    fn parse_missing_url_errors() {
        let error = parse("GET").unwrap_err();

        assert_eq!(error.line, 1);
        assert_eq!(error.message, "Missing request URL");
    }
}

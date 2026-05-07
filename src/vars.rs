use crate::parser::{Method, ParsedRequest, Variable};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRequest {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VarError {
    pub variable_name: String,
    pub field: String,
}

impl fmt::Display for VarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Undefined variable '{}' in {}",
            self.variable_name, self.field
        )
    }
}

pub fn resolve(
    variables: &[Variable],
    request: &ParsedRequest,
) -> Result<ResolvedRequest, VarError> {
    let values: HashMap<&str, &str> = variables
        .iter()
        .map(|variable| (variable.name.as_str(), variable.value.as_str()))
        .collect();

    let url = resolve_field(&request.url, &values, "url")?;
    let headers = request
        .headers
        .iter()
        .map(|(name, value)| {
            Ok((
                resolve_field(name, &values, "header")?,
                resolve_field(value, &values, "header")?,
            ))
        })
        .collect::<Result<Vec<_>, VarError>>()?;
    let body = request
        .body
        .as_deref()
        .map(|value| resolve_field(value, &values, "body"))
        .transpose()?;

    Ok(ResolvedRequest {
        method: request.method.clone(),
        url,
        headers,
        body,
    })
}

fn resolve_field(
    input: &str,
    variables: &HashMap<&str, &str>,
    field: &str,
) -> Result<String, VarError> {
    let mut result = String::with_capacity(input.len());
    let mut cursor = 0;

    while let Some(open_offset) = input[cursor..].find("{{") {
        let open_index = cursor + open_offset;
        result.push_str(&input[cursor..open_index]);

        let name_start = open_index + 2;
        if let Some(close_offset) = input[name_start..].find("}}") {
            let close_index = name_start + close_offset;
            let variable_name = &input[name_start..close_index];
            let value = variables.get(variable_name).ok_or_else(|| VarError {
                variable_name: variable_name.to_string(),
                field: field.to_string(),
            })?;

            result.push_str(value);
            cursor = close_index + 2;
        } else {
            result.push_str(&input[open_index..]);
            return Ok(result);
        }
    }

    result.push_str(&input[cursor..]);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{Method, ParsedRequest, Variable};

    fn variable(name: &str, value: &str) -> Variable {
        Variable {
            name: name.to_string(),
            value: value.to_string(),
        }
    }

    fn request(url: &str) -> ParsedRequest {
        ParsedRequest {
            name: Some("example".to_string()),
            method: Method::Post,
            url: url.to_string(),
            headers: vec![("Authorization".to_string(), "Bearer static".to_string())],
            body: Some("default body".to_string()),
            source_line: 42,
        }
    }

    #[test]
    fn resolve_simple_url_substitution() {
        let request = request("{{host}}/path");

        let resolved = resolve(&[variable("host", "https://api.com")], &request).unwrap();

        assert_eq!(resolved.url, "https://api.com/path");
    }

    #[test]
    fn resolve_multiple_vars_in_url() {
        let request = request("{{host}}/{{version}}/users");

        let resolved = resolve(
            &[
                variable("host", "https://api.com"),
                variable("version", "v1"),
            ],
            &request,
        )
        .unwrap();

        assert_eq!(resolved.url, "https://api.com/v1/users");
    }

    #[test]
    fn resolve_var_in_header_value() {
        let mut request = request("https://api.com/users");
        request.headers = vec![(
            "Authorization".to_string(),
            "Bearer {{token}}".to_string(),
        )];

        let resolved = resolve(&[variable("token", "abc123")], &request).unwrap();

        assert_eq!(
            resolved.headers,
            vec![("Authorization".to_string(), "Bearer abc123".to_string())]
        );
    }

    #[test]
    fn resolve_var_in_body() {
        let mut request = request("https://api.com/users");
        request.body = Some("{\"username\":\"{{username}}\"}".to_string());

        let resolved = resolve(&[variable("username", "alice")], &request).unwrap();

        assert_eq!(resolved.body, Some("{\"username\":\"alice\"}".to_string()));
    }

    #[test]
    fn resolve_multiple_vars_in_body() {
        let mut request = request("https://api.com/users");
        request.body = Some(
            "{\"user\":\"{{username}}\",\"role\":\"{{role}}\",\"env\":\"{{env}}\"}"
                .to_string(),
        );

        let resolved = resolve(
            &[
                variable("username", "alice"),
                variable("role", "admin"),
                variable("env", "prod"),
            ],
            &request,
        )
        .unwrap();

        assert_eq!(
            resolved.body,
            Some("{\"user\":\"alice\",\"role\":\"admin\",\"env\":\"prod\"}".to_string())
        );
    }

    #[test]
    fn resolve_undefined_variable_error() {
        let request = request("{{unknown}}/path");

        let error = resolve(&[], &request).unwrap_err();

        assert_eq!(
            error,
            VarError {
                variable_name: "unknown".to_string(),
                field: "url".to_string(),
            }
        );
    }

    #[test]
    fn resolve_error_reports_field_url() {
        let request = request("{{missing}}/path");

        let error = resolve(&[], &request).unwrap_err();

        assert_eq!(error.field, "url");
    }

    #[test]
    fn resolve_error_reports_field_header() {
        let mut request = request("https://api.com");
        request.headers = vec![("X-Test".to_string(), "{{missing}}".to_string())];

        let error = resolve(&[], &request).unwrap_err();

        assert_eq!(error.field, "header");
    }

    #[test]
    fn resolve_error_reports_field_body() {
        let mut request = request("https://api.com");
        request.body = Some("{{missing}}".to_string());

        let error = resolve(&[], &request).unwrap_err();

        assert_eq!(error.field, "body");
    }

    #[test]
    fn resolve_no_vars_passthrough() {
        let request = ParsedRequest {
            name: Some("plain".to_string()),
            method: Method::Get,
            url: "https://api.com/users".to_string(),
            headers: vec![("Accept".to_string(), "application/json".to_string())],
            body: Some("plain body".to_string()),
            source_line: 3,
        };

        let resolved = resolve(&[], &request).unwrap();

        assert_eq!(
            resolved,
            ResolvedRequest {
                method: Method::Get,
                url: "https://api.com/users".to_string(),
                headers: vec![("Accept".to_string(), "application/json".to_string())],
                body: Some("plain body".to_string()),
            }
        );
    }

    #[test]
    fn resolve_var_value_with_braces_not_recursive() {
        let request = request("{{outer}}/path");

        let resolved = resolve(&[variable("outer", "{{other}}")], &request).unwrap();

        assert_eq!(resolved.url, "{{other}}/path");
    }

    #[test]
    fn resolve_empty_variables_with_no_refs() {
        let request = request("https://api.com/status");

        let resolved = resolve(&[], &request).unwrap();

        assert_eq!(resolved.url, "https://api.com/status");
        assert_eq!(resolved.headers, request.headers);
        assert_eq!(resolved.body, request.body);
    }

    #[test]
    fn resolve_case_sensitive() {
        let request = request("{{Host}}/path");

        let error = resolve(&[variable("host", "https://api.com")], &request).unwrap_err();

        assert_eq!(error.variable_name, "Host");
    }

    #[test]
    fn resolve_none_body_passthrough() {
        let mut request = request("https://api.com/users");
        request.body = None;

        let resolved = resolve(&[], &request).unwrap();

        assert_eq!(resolved.body, None);
    }
}

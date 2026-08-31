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
pub enum VarError {
    Undefined {
        variable_name: String,
        field: String,
    },
    Circular {
        chain: String,
    },
}

impl fmt::Display for VarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VarError::Undefined {
                variable_name,
                field,
            } => write!(f, "Undefined variable '{variable_name}' in {field}"),
            VarError::Circular { chain } => write!(f, "Circular variable reference: {chain}"),
        }
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

    let mut resolver = Resolver {
        values,
        resolved: HashMap::new(),
    };

    let url = resolver.substitute(&request.url, "url")?;
    let headers = request
        .headers
        .iter()
        .map(|(name, value)| {
            Ok((
                resolver.substitute(name, "header")?,
                resolver.substitute(value, "header")?,
            ))
        })
        .collect::<Result<Vec<_>, VarError>>()?;
    let body = request
        .body
        .as_deref()
        .map(|value| resolver.substitute(value, "body"))
        .transpose()?;

    Ok(ResolvedRequest {
        method: request.method.clone(),
        url,
        headers,
        body,
    })
}

struct Resolver<'a> {
    values: HashMap<&'a str, &'a str>,
    resolved: HashMap<&'a str, String>,
}

impl<'a> Resolver<'a> {
    // Replaces each {{name}} with the variable's fully resolved value
    // (recursively resolving references inside values).
    fn substitute(&mut self, input: &'a str, field: &str) -> Result<String, VarError> {
        self.substitute_in(input, field, &mut Vec::new())
    }

    fn substitute_in(
        &mut self,
        input: &'a str,
        field: &str,
        path: &mut Vec<&'a str>,
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
                let value = self.resolve_variable(variable_name, field, path)?;
                result.push_str(&value);
                cursor = close_index + 2;
            } else {
                result.push_str(&input[open_index..]);
                return Ok(result);
            }
        }

        result.push_str(&input[cursor..]);
        Ok(result)
    }

    // Resolves one variable to its final value; memoized. `path` holds the
    // reference chain currently being resolved, so a revisit is a cycle.
    // It must thread through substitute_in — a fresh path per level would
    // miss self-references and recurse until the stack overflows.
    fn resolve_variable(
        &mut self,
        name: &'a str,
        field: &str,
        path: &mut Vec<&'a str>,
    ) -> Result<String, VarError> {
        if let Some(done) = self.resolved.get(name) {
            return Ok(done.clone());
        }
        let raw = match self.values.get(name) {
            Some(raw) => *raw,
            None => {
                return Err(VarError::Undefined {
                    variable_name: name.to_string(),
                    field: field.to_string(),
                });
            }
        };
        if path.contains(&name) {
            let mut chain: Vec<String> = path.iter().map(|n| n.to_string()).collect();
            chain.push(name.to_string());
            return Err(VarError::Circular {
                chain: chain.join(" → "),
            });
        }
        path.push(name);
        let value = self.substitute_in(raw, field, path)?;
        path.pop();
        self.resolved.insert(name, value.clone());
        Ok(value)
    }
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
        request.headers = vec![("Authorization".to_string(), "Bearer {{token}}".to_string())];

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
            "{\"user\":\"{{username}}\",\"role\":\"{{role}}\",\"env\":\"{{env}}\"}".to_string(),
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
            VarError::Undefined {
                variable_name: "unknown".to_string(),
                field: "url".to_string(),
            }
        );
    }

    #[test]
    fn resolve_error_reports_field_url() {
        let request = request("{{missing}}/path");

        let error = resolve(&[], &request).unwrap_err();

        assert_eq!(
            error,
            VarError::Undefined {
                variable_name: "missing".to_string(),
                field: "url".to_string()
            }
        );
    }

    #[test]
    fn resolve_error_reports_field_header() {
        let mut request = request("https://api.com");
        request.headers = vec![("X-Test".to_string(), "{{missing}}".to_string())];

        let error = resolve(&[], &request).unwrap_err();

        assert_eq!(
            error,
            VarError::Undefined {
                variable_name: "missing".to_string(),
                field: "header".to_string()
            }
        );
    }

    #[test]
    fn resolve_error_reports_field_body() {
        let mut request = request("https://api.com");
        request.body = Some("{{missing}}".to_string());

        let error = resolve(&[], &request).unwrap_err();

        assert_eq!(
            error,
            VarError::Undefined {
                variable_name: "missing".to_string(),
                field: "body".to_string()
            }
        );
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
    fn resolve_transitive_variable_chain() {
        let variables = vec![
            variable("prdIngress", "prod.ingress.domain.name"),
            variable("remoteServer", "https://{{prdIngress}}/context-path"),
            variable("baseUrl", "{{remoteServer}}"),
        ];
        let resolved = resolve(&variables, &request("{{baseUrl}}/ping")).unwrap();
        assert_eq!(
            resolved.url,
            "https://prod.ingress.domain.name/context-path/ping"
        );
    }

    #[test]
    fn resolve_transitive_variables_in_headers_and_body() {
        let variables = vec![
            variable("host", "api.internal"),
            variable("origin", "https://{{host}}"),
        ];
        let mut req = request("{{origin}}/v1");
        req.headers = vec![("X-Base".to_string(), "{{origin}}/v1".to_string())];
        req.body = Some("server={{origin}}".to_string());
        let resolved = resolve(&variables, &req).unwrap();
        assert_eq!(resolved.url, "https://api.internal/v1");
        assert_eq!(resolved.headers[0].1, "https://api.internal/v1");
        assert_eq!(
            resolved.body.as_deref(),
            Some("server=https://api.internal")
        );
    }

    #[test]
    fn resolve_same_transitive_variable_reused_across_fields() {
        let variables = vec![
            variable("host", "api.internal"),
            variable("origin", "https://{{host}}"),
        ];
        let mut req = request("{{origin}}/a");
        req.headers = vec![("X-Base".to_string(), "{{origin}}".to_string())];
        let resolved = resolve(&variables, &req).unwrap();
        assert_eq!(resolved.url, "https://api.internal/a");
        assert_eq!(resolved.headers[0].1, "https://api.internal");
    }

    #[test]
    fn resolve_circular_variable_references_error() {
        let variables = vec![variable("a", "{{b}}/x"), variable("b", "{{a}}/y")];
        let error = resolve(&variables, &request("{{a}}/ping")).unwrap_err();
        match error {
            VarError::Circular { chain } => {
                assert!(chain.contains('a') && chain.contains('b'), "chain: {chain}");
            }
            other => panic!("expected circular reference error, got {other:?}"),
        }
    }

    #[test]
    fn resolve_self_referencing_variable_errors() {
        let error = resolve(&[variable("a", "{{a}}")], &request("{{a}}/x")).unwrap_err();
        assert!(matches!(error, VarError::Circular { .. }));
    }

    #[test]
    fn resolve_undefined_inside_variable_value_errors() {
        let error = resolve(
            &[variable("outer", "{{missing}}")],
            &request("{{outer}}/path"),
        )
        .unwrap_err();
        assert_eq!(
            error,
            VarError::Undefined {
                variable_name: "missing".to_string(),
                field: "url".to_string()
            }
        );
    }

    #[test]
    fn resolve_unused_variable_with_unknown_reference_ok() {
        let variables = vec![variable("used", "https://ok"), variable("junk", "{{nope}}")];
        let resolved = resolve(&variables, &request("{{used}}/x")).unwrap();
        assert_eq!(resolved.url, "https://ok/x");
    }

    #[test]
    fn resolve_sibling_references_share_no_path() {
        let variables = vec![variable("x", "xx")];
        let resolved = resolve(&variables, &request("{{x}}{{x}}")).unwrap();
        assert_eq!(resolved.url, "xxxx");
    }

    #[test]
    fn resolve_diamond_references_do_not_false_cycle() {
        let variables = vec![
            variable("a", "{{b}}{{c}}"),
            variable("b", "{{d}}"),
            variable("c", "{{d}}"),
            variable("d", "x"),
        ];
        let resolved = resolve(&variables, &request("{{a}}")).unwrap();
        assert_eq!(resolved.url, "xx");
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

        assert_eq!(
            error,
            VarError::Undefined {
                variable_name: "Host".to_string(),
                field: "url".to_string()
            }
        );
    }

    #[test]
    fn resolve_none_body_passthrough() {
        let mut request = request("https://api.com/users");
        request.body = None;

        let resolved = resolve(&[], &request).unwrap();

        assert_eq!(resolved.body, None);
    }
}

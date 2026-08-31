# Transitive Variables + Error Diagnosability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Support chained/transitive variables (`@a={{b}}` where `b` itself contains `{{c}}`) in `.http` files — the root cause of the user's `request failed: builder error` — and make reqwest errors self-explanatory by including their source chain.

**Architecture:** `vars.rs` gains a memoized recursive `Resolver` (substitute ↔ resolve-variable mutual recursion, cycle detection via path membership). `http.rs` gains a `format_error_chain` helper that walks `std::error::Error::source()` so builder errors name their real cause. Ships as 0.1.1 (first OIDC-pipeline release).

**Tech Stack:** Rust (no new dependencies), wiremock/existing test infra.

**Root cause (confirmed):** `src/vars.rs` `resolve_field` is single-pass — `{{baseUrl}}` expands to the raw value `{{remoteServer}}` which is never re-scanned; the literal braces end up in the URL and `Url::parse` fails inside reqwest → Builder-kind error, whose source `http.rs:56` drops via bare `{error}` Display.

**Conventions:** Commit style imperative, no prefixes. TDD: failing test → implement → green. Work happens in worktree `.worktrees/transitive-vars` on branch `feat/transitive-variables`.

**Design decisions locked:**
- Recursive resolution is **lazy**: only variables reachable from url/headers/body get resolved (unused messy variables never error)
- Cycle detection via path membership — no depth cap needed (the definition graph is finite; a non-cyclic chain is bounded by the variable count; a cap would be unreachable code)
- A `{{ref}}` inside a used variable's value that names no variable yields the same `Undefined` error as one written directly in a URL, with the leaf name and the triggering field
- `VarError` becomes an enum: `Undefined { variable_name, field }` (Display unchanged) + `Circular { chain }` ("Circular variable reference: a → b → a")
- Errors raised while resolving a variable's value carry the field that triggered resolution (`url`/`header`/`body`)

**File map:**

```
src/http.rs        # modify: format_error_chain + both map_err sites + test update
src/vars.rs        # modify: VarError enum, Resolver, new tests, flip single-pass test
tests/integration.rs  # modify: pattern-match VarError in undefined test
README.md          # modify: feature bullet
AGENTS.md          # modify: refresh the stale single-pass example comment
Cargo.toml         # modify: version 0.1.0 → 0.1.1
```

---

### Task 1: Error source chain in http.rs (TDD)

**Files:**
- Modify: `src/http.rs`

- [ ] **Step 1: Strengthen the failing test**

Replace `send_connection_error` (http.rs:345-357) with:

```rust
    #[tokio::test]
    async fn send_connection_error_includes_source_chain() {
        let error = send_via_blocking_client(resolved_request(
            Method::Get,
            "not a valid url".to_string(),
            vec![],
            None,
        ))
        .await
        .unwrap_err();

        assert!(error.message.contains("request failed"));
        assert!(error.message.contains("builder error"));
        assert!(error.message.contains("relative URL"));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test send_connection_error`
Expected: FAIL — message is `request failed: builder error`, missing "relative URL".

- [ ] **Step 3: Implement**

Add near the top of `src/http.rs` (after imports):

```rust
fn format_error_chain(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(current) = source {
        message.push_str(": ");
        message.push_str(&current.to_string());
        source = current.source();
    }
    message
}
```

Change both error sites:
- line ~55: `message: format!("request failed: {error}")` → `message: format!("request failed: {}", format_error_chain(&error))`
- line ~77: `message: format!("failed to read response body: {error}")` → `message: format!("failed to read response body: {}", format_error_chain(&error))`

(If the actual source text for `"not a valid url"` differs from "relative URL without a base", run the test, read the real chain from the failure output, and adjust the third assertion to the observed source — but keep asserting BOTH the reqwest kind ("builder error") AND a non-empty source fragment.)

- [ ] **Step 4: Run to verify pass + full suite**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/http.rs
git commit -m "Include error source chain in request failure messages"
```

---

### Task 2: Transitive variables in vars.rs (TDD)

**Files:**
- Modify: `src/vars.rs`
- Modify: `tests/integration.rs:94-102`
- Modify: `README.md` (Features bullet)
- Modify: `AGENTS.md` (stale example comment)

- [ ] **Step 1: Write the failing tests**

In `src/vars.rs` tests module, add:

```rust
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
        assert_eq!(resolved.body.as_deref(), Some("server=https://api.internal"));
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
        let variables = vec![
            variable("a", "{{b}}/x"),
            variable("b", "{{a}}/y"),
        ];
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
        let error =
            resolve(&[variable("outer", "{{missing}}")], &request("{{outer}}/path")).unwrap_err();
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
        let variables = vec![
            variable("used", "https://ok"),
            variable("junk", "{{nope}}"),
        ];
        let resolved = resolve(&variables, &request("{{used}}/x")).unwrap();
        assert_eq!(resolved.url, "https://ok/x");
    }
```

Replace the existing single-pass test (vars.rs ~line 260, asserting `resolved.url == "{{other}}/path"`) — its expectation flips by design; the replacement is `resolve_undefined_inside_variable_value_errors` above. Delete the old test.

- [ ] **Step 2: Run to verify failures**

Run: `cargo test --lib vars`
Expected: compile errors (no `VarError::Circular`, no recursive resolution) — the red state.

- [ ] **Step 3: Implement**

Replace the `VarError` struct + `resolve` + `resolve_field` block (vars.rs:13-94) with:

```rust
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
    // Scans input once, replacing every {{name}} with the variable's fully
    // resolved value (recursively resolving references inside values).
    fn substitute(&mut self, input: &'a str, field: &str) -> Result<String, VarError> {
        let mut result = String::with_capacity(input.len());
        let mut cursor = 0;

        while let Some(open_offset) = input[cursor..].find("{{") {
            let open_index = cursor + open_offset;
            result.push_str(&input[cursor..open_index]);

            let name_start = open_index + 2;
            if let Some(close_offset) = input[name_start..].find("}}") {
                let close_index = name_start + close_offset;
                let variable_name = &input[name_start..close_index];
                let value = self.resolve_variable(variable_name, field, &mut Vec::new())?;
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
                })
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
        let value = self.substitute(raw, field)?;
        path.pop();
        self.resolved.insert(name, value.clone());
        Ok(value)
    }
}
```

Add `HashSet`? Not needed — remove from imports if unused; keep `use std::collections::HashMap;` and drop `use std::collections::HashSet;` if the old file didn't have it (it didn't).

- [ ] **Step 4: Update integration test** (`tests/integration.rs:94-102`) — struct field access becomes pattern equality:

```rust
#[test]
fn test_undefined_variable_error_propagation() {
    let input = "GET {{host}}/users\nAuthorization: Bearer {{undefined}}";
    let parsed = parser::parse(input).expect("request file should parse");
    let error = vars::resolve(&parsed.variables, &parsed.requests[0])
        .expect_err("undefined variable should fail resolution");

    assert_eq!(
        error,
        vars::VarError::Undefined {
            variable_name: "host".to_string(),
            field: "url".to_string()
        }
    );
}
```

- [ ] **Step 5: Full suite green**

Run: `cargo test`
Expected: all pass (app.rs:404 asserts the Undefined Display string — unchanged by design).

- [ ] **Step 6: Docs**

- `README.md` Features bullet: "Resolve `{{variables}}` in URLs, headers, and bodies" → "Resolve `{{variables}}` in URLs, headers, and bodies, including chained variables (`@a = {{b}}`)"
- `AGENTS.md` Comments section: refresh the stale example — replace "(e.g., variable substitution is single-pass, so `{{...}}` in a variable's value is left literal)" with "(e.g., variable resolution is recursive with cycle detection, so a looping `{{...}}` chain reports the full cycle in its error)". Leave the rest of the file untouched.

- [ ] **Step 7: Commit**

```bash
git add src/vars.rs tests/integration.rs README.md AGENTS.md
git commit -m "Resolve chained variables recursively with cycle detection"
```

---

### Task 3: Version bump + full gate

**Files:**
- Modify: `Cargo.toml:3` (`version = "0.1.0"` → `"0.1.1"`)

- [ ] **Step 1: Bump version, verify stamped everywhere**

```bash
# edit Cargo.toml version to 0.1.1
cargo test --quiet   # regenerates lockfile usage; Cargo.lock version field updates
node npm/scripts/stage.mjs --help >/dev/null 2>&1 || true
make npm-test 2>&1 | tail -3        # 7/7 (uses Cargo.toml version dynamically)
```

- [ ] **Step 2: Full local gate**

Run: `make lint && cargo test && make npm-test && make npm-test-local`
Expected: all green; e2e PASS line shows `restui@0.1.1`.

- [ ] **Step 3: Manual smoke with the user's exact chain (human, after merge)**

```bash
printf '@prdIngress=prod.ingress.domain.name\n@remoteServer=https://{{prdIngress}}/context-path\n@baseUrl={{remoteServer}}\n\n### ping\nGET {{baseUrl}}/ping HTTP/1.1\n' > /tmp/chain.http
cargo run -- /tmp/chain.http
```

Press `d` → confirm the resolved URL shows `https://prod.ingress.domain.name/context-path/ping`; Enter against a reachable host succeeds (a fake host shows the now-detailed DNS error, not "builder error").

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "Bump version to 0.1.1"
```

---

## Self-review notes

- **Spec coverage:** root cause (single-pass) → Task 2; diagnosability → Task 1; user's exact file → Task 2 first test + Task 3 manual smoke; behavior changes (enum, flipped test) → Task 2 Steps 1/4; docs truthfulness → Task 2 Step 6 + AGENTS.md.
- **Type consistency:** `VarError::Undefined` field names identical to old struct (`variable_name`, `field`) so Display and app.rs:404 assertion stay byte-identical; `Circular` used identically in tests and Display.
- **Placeholders:** none — every step has complete code and expected output.
- **Dropped from proposal:** depth cap — provably unreachable with correct cycle detection over a finite variable set (would be dead code).

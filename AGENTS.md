# AGENTS.md

Guidance for coding agents (and humans) working in this repository.

## Versioning

Never bump the version in `Cargo.toml` unless explicitly asked. Releases are cut manually by the maintainer: implement, verify, and leave the version untouched.

## Comments

Prefer self-documenting code over comments. Only add comments for knowledge that cannot be expressed in the code itself.

- **Self-document first.** Use clear test names, descriptive assertion values, and meaningful function names. A `match` expression or an `assert_eq!` in a test is already its own documentation.
- **Never restate what the code says.** If a comment describes what the next line does, the code already says that — remove the comment.
- **Never reference change history.** Comments like `// After the refactor, ...` are git-history noise. The commit message already captures what changed. Explain the *current state*, not the transition.
- **Reserve comments for non-obvious knowledge only:** gotchas that look correct but aren't (e.g., an em-dash resembling a hyphen), design decisions not evident from the code (e.g., variable resolution is recursive with cycle detection, so a looping `{{...}}` chain reports the full cycle in its error), and contract invariants a maintainer might unknowingly violate (e.g., "assumes the request is already parsed").
- **Keep doc comments short.** If a doc comment exceeds 5 lines, it is likely restating the function body. Trim to the contract and non-obvious behaviors.

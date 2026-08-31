# npm README Sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The npm package README is derived from the repo README at staging time (single source), with relative links/images rewritten to absolute GitHub URLs so they work on npmjs.com.

**Architecture:** Delete the hand-maintained `npm/restui/README.md` template. `stage.mjs` reads the repo `README.md` (new optional `--readme` flag, default repo root), applies `absolutizeReadmeLinks` (images → `raw.githubusercontent.com`, other relative links → `github.com/blob/main`, absolute/anchor/mailto untouched), and writes the result into the staged main package. The npm-only "Node 14+" note moves into the repo README. Ships as 0.1.2.

**Tech Stack:** Node ≥18 stdlib (stage.mjs), node:test. No new dependencies.

**Design decisions locked:**
- Single source: repo `README.md` is the only editable document; drift is structurally impossible
- Images use `raw.githubusercontent.com` (github.com blob URLs serve HTML, which does not render as an image); text links use `blob/main` (renders nicely on GitHub)
- The npm README's old inline troubleshooting is superseded by the absolutized Documentation links (progressive disclosure holds; `docs/tls.md` is reachable from npm)
- Link targets containing spaces/titles (`![x](url "t")`) are out of scope — none exist; the transform's regex `[^)\s]+` deliberately skips them (leaves them untouched rather than corrupting)
- `--readme` flag mirrors the `--templates-dir` precedent: hermetic transform tests

**File map:**

```
npm/scripts/stage.mjs        # modify: absolutizeReadmeLinks + --readme flag + main staging writes generated README
npm/scripts/stage.test.mjs   # modify: 2 new tests
npm/restui/README.md         # DELETE (template superseded)
README.md                    # modify: Node 14+ note in Installation
docs/superpowers/specs/2026-08-27-npm-publishing-design.md  # modify: §1/§2 reflect generated README
Cargo.toml / Cargo.lock      # modify: 0.1.1 → 0.1.2
```

---

### Task 1: README generation in stage.mjs (TDD)

**Files:**
- Test: `npm/scripts/stage.test.mjs`
- Modify: `npm/scripts/stage.mjs`
- Delete: `npm/restui/README.md`

- [ ] **Step 1: Write the failing tests**

Add to `npm/scripts/stage.test.mjs` (constants `BLOB = 'https://github.com/will8ug/restui/blob/main/'` and `RAW = 'https://raw.githubusercontent.com/will8ug/restui/main/'` at top of tests):

```js
test('main package README is generated from the repo README with absolute links', async () => {
  await withTemp(async (dir) => {
    const binaries = path.join(dir, 'artifacts');
    const out = path.join(dir, 'out');
    await makeFakeBinaries(binaries);
    const res = runStage(['--binaries-dir', binaries, '--out', out]);
    assert.equal(res.status, 0, `stderr: ${res.stderr}`);
    const readme = await readFile(path.join(out, 'restui', 'README.md'), 'utf8');
    const source = await readFile(path.join(REPO, 'README.md'), 'utf8');
    for (const [, bang, target] of source.matchAll(/(!?)\]\(([^)\s]+)\)/g)) {
      if (/^(https?:|mailto:|#|\/)/.test(target)) continue;
      const base = bang ? 'https://raw.githubusercontent.com/will8ug/restui/main/' : 'https://github.com/will8ug/restui/blob/main/';
      assert.ok(readme.includes(`](${base}${target})`), `missing absolutized link for ${target}`);
    }
    assert.ok(!/\]\((docs|assets)\//.test(readme), 'relative docs/assets links must not remain');
  });
});

test('README transform rewrites only relative links', async () => {
  await withTemp(async (dir) => {
    const readmePath = path.join(dir, 'README.md');
    await writeFile(
      readmePath,
      [
        '# t',
        '',
        '![shot](assets/shot.png)',
        '[docs](docs/tls.md)',
        '[anchor](#section)',
        '[web](https://example.com/x)',
        '[mail](mailto:a@b.c)',
        '[abs](/absolute/path)',
        '',
      ].join('\n')
    );
    const binaries = path.join(dir, 'artifacts');
    const out = path.join(dir, 'out');
    await makeFakeBinaries(binaries);
    const res = runStage(['--binaries-dir', binaries, '--out', out, '--readme', readmePath]);
    assert.equal(res.status, 0, `stderr: ${res.stderr}`);
    const readme = await readFile(path.join(out, 'restui', 'README.md'), 'utf8');
    assert.ok(readme.includes('![shot](https://raw.githubusercontent.com/will8ug/restui/main/assets/shot.png)'));
    assert.ok(readme.includes('[docs](https://github.com/will8ug/restui/blob/main/docs/tls.md)'));
    assert.ok(readme.includes('[anchor](#section)'));
    assert.ok(readme.includes('[web](https://example.com/x)'));
    assert.ok(readme.includes('[mail](mailto:a@b.c)'));
    assert.ok(readme.includes('[abs](/absolute/path)'));
  });
});
```

- [ ] **Step 2: Run to verify failures**

`node --test 'npm/scripts/*.test.mjs'` — Expected: 2 new tests FAIL (test 1: staged README is the old template copy, missing absolutized links; test 2: `--readme` is an unknown argument → exit 2). The existing 9 tests still pass.

- [ ] **Step 3: Implement**

In `npm/scripts/stage.mjs`:

3a. Add constants after `MIN_BINARY_BYTES`:

```js
const GITHUB_BLOB_BASE = 'https://github.com/will8ug/restui/blob/main/';
const GITHUB_RAW_BASE = 'https://raw.githubusercontent.com/will8ug/restui/main/';
```

3b. Add the transform (single source of truth is the repo README; images must use raw URLs because github.com blob URLs serve HTML, which does not render as an image on npm):

```js
function absolutizeReadmeLinks(markdown) {
  return markdown.replace(/(!?)\]\(([^)\s]+)\)/g, (match, bang, target) => {
    if (/^(https?:|mailto:|#|\/)/.test(target)) return match;
    const base = bang ? GITHUB_RAW_BASE : GITHUB_BLOB_BASE;
    return `${bang}](${base}${target})`;
  });
}
```

3c. `parseArgs`: add case `else if (argv[i] === '--readme') args.readme = argv[++i];` and default `readme: path.join(REPO, 'README.md')` in the args object.

3d. In `main()`'s main-package staging, REPLACE `await copyFile(path.join(mainSrc, 'README.md'), path.join(mainDest, 'README.md'));` with:

```js
  const readme = absolutizeReadmeLinks(await readFile(args.readme, 'utf8'));
  await writeFile(path.join(mainDest, 'README.md'), readme);
```

3e. Delete `npm/restui/README.md` (`git rm npm/restui/README.md`).

- [ ] **Step 4: Run to verify pass**

`node --test 'npm/scripts/*.test.mjs'` — Expected: 11/11 pass. Then `make npm-test-local` (full e2e — the main package now stages the generated README; pack still succeeds because npm auto-includes README regardless of `files`).

- [ ] **Step 5: Commit**

```bash
git add npm/scripts/stage.mjs npm/scripts/stage.test.mjs
git rm npm/restui/README.md 2>/dev/null || true   # already staged if done in 3e
git commit -m "Generate npm package README from the repo README"
```

---

### Task 2: Node 14+ note + doc truthfulness

**Files:**
- Modify: `README.md`
- Modify: `docs/superpowers/specs/2026-08-27-npm-publishing-design.md`

- [ ] **Step 1: README Installation note**

In `README.md`, immediately after the npm fenced code block (before `**cargo**`), add:

```markdown
The npm-installed shim requires Node 14+; cargo installs have no Node requirement.
```

- [ ] **Step 2: Design spec updates**

In `docs/superpowers/specs/2026-08-27-npm-publishing-design.md`:
- §1 package table, `restui` row: contents "JS bin shim, README, LICENSE" → "JS bin shim, README (generated from repo README at stage time), LICENSE"
- §2 repository-layout tree: remove the `restui/README.md` line; adjust the `restui/` comment if it mentions README
- Add one sentence near the §2 layout: "The main package's README is not a template — `stage.mjs` derives it from the repo `README.md`, rewriting relative links/images to absolute GitHub URLs (raw for images) so they render on npmjs.com."

- [ ] **Step 3: Verify + commit**

`node --test 'npm/scripts/*.test.mjs'` still 11/11 (test 1 consumes the edited README — the new note line contains no links, so it stays green). Commit:

```bash
git add README.md docs/superpowers/specs/2026-08-27-npm-publishing-design.md
git commit -m "Note Node requirement and document generated npm README"
```

---

### Task 3: Version bump 0.1.2 + full gate

**Files:**
- Modify: `Cargo.toml:3` → `"0.1.2"`

- [ ] **Step 1: Bump + gate**

```bash
# edit Cargo.toml version to 0.1.2
make lint && cargo test && make npm-test && make npm-test-local
```

Expected: all green; e2e PASS line shows `restui@0.1.2`. Inspect `target/npm/restui/README.md` — must show absolutized links and the Node 14+ note.

- [ ] **Step 2: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "Bump version to 0.1.2"
```

- [ ] **Step 3: Post-merge human verification (documented, not automated)**

After release: npmjs.com/package/@will8ug/restui shows screenshots rendering and Documentation links pointing at GitHub.

---

## Self-review notes

- **Spec coverage:** single-source generation (Task 1), npm-only note relocation + doc truth (Task 2), release vehicle (Task 3).
- **Consistency:** transform regex identical in stage.mjs and test 1's mirror loop; `--readme` default mirrors REPO anchoring; platform READMEs untouched (they are one-liners, not synced).
- **Placeholders:** none — complete code and expected outputs throughout.

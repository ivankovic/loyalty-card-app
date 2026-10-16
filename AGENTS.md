# Development Workflow

- Always read the README.md file in the root of the repository. Always.
- Always read README.md in any directory in this repository before you read or write any files in
that directory.

## Branches

- Until v0.1.0, commit straight to `main`: Marko is the only developer. No branches, no pull
requests. Push only when Marko says so.
- Releases are cut from `main`.

## Markdown files

- Do NOT update the README.md files unless explicitly asked to do so.
- There are no TODO.md or SPECS.md files; do not create them. Working notes - follow-ups,
experiments and their numbers, negative results, and design decisions with their reasons - go in
AGENT_LOG.md at the repository root, which is git-ignored and never committed. Read it before
starting a task; when you complete an item from it, mark it done there.
- A design decision that a reader of the code needs belongs next to the code, in its module-level
doc comment (`//!` in Rust, the opening comment in TypeScript), not in a separate document.
- Always clean up REVIEW.md when you complete a task from it.

# Tests

- Read "Testing" in CONTRIBUTING.md before writing one: the time budgets, no mocks, no network.
- A test should read as a claim, not a script: its name says what is true, its body shows it.
- Every assertion should say what happened when it fails: add the value to the message.
- Pin generated output (the schema, `--help`, the API's JSON, a drawn barcode) with a golden
rather than a wall of substring checks.
- The e2e tests find things the way a person does: by label, role and text, never by a CSS path
when a name would do.

# Rust

- Output goes through the command's `out` (CLI) or the request log (server); a stray `println!`
is a bug.
- Database work runs through `AppState::with_db`, never on an async thread.

# TypeScript

- No UI framework: `h()` from `src/dom.ts` builds the page. No inline `style=` or `<script>`: the
CSP forbids both; set custom properties with `element.style.setProperty`.
- Every dependency in `dependencies` ships to every phone and needs a line in
THIRD-PARTY-NOTICES.md (`make third-party-notices`). Think twice.

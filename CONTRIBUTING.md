# Contributing

Contributions are welcome, and bug reports most of all.

## Reporting a bug

[Open an issue](https://github.com/ivankovic/loyalty-card-app/issues). The most useful reports
say:

- which version (`loyalty --version`, or the line at the bottom of the app's settings);
- which phone and browser, and whether the app was installed to the home screen;
- what you did, what you expected, and what happened instead. For a code that does not scan or
  does not read at a till, its kind and, if you can share it, a photo.

A security problem is the exception: don't open a public issue, follow
[`SECURITY.md`](SECURITY.md) instead.

## Sending a change

1. For anything bigger than a small fix, open an issue first so we can agree on the approach
   before you spend time on it.
2. Follow the rest of this file. In practice: `make hooks` once, so formatting runs on every
   commit and the linters and unit tests on every push, and `make ci` before you open the pull
   request.
3. Agree to the [Contributor License Agreement](CLA.md) by posting the comment it asks for on
   your pull request. You keep the copyright in your work; the CLA lets the project offer a
   commercial license alongside the AGPL (see "License" in the README). It is needed once, and
   only for changes, not for bug reports.

## How the project works

The rest of this file is everything a contributor, the maintainer or a coding agent needs to
change the code with confidence: the technology, the rules for quality and tests, and the
layout.

## Technology

- **The server** is Rust: axum on tokio, behind its own accept loop (`src/web/serve.rs`);
  SQLite through rusqlite, built into the binary. One process, one database file, one
  connection behind a mutex. It serves a JSON API, the photos, and the web app, which
  `build.rs` embeds from `web/dist`.
- **The web app** is TypeScript without a UI framework, bundled by Bun. It reads and draws
  barcodes with zxing-wasm (zxing-cpp and zint compiled to WebAssembly), keeps its last data in
  IndexedDB and its files in a service worker, so that it opens offline.
- **Tooling** is cargo and Bun. There is no Python, no npm and no Node in this project; Bun
  runs the scripts in `scripts/`.

## Code quality

- Rust: `cargo fmt`, and `cargo clippy --all-targets -- -D warnings` with no warnings. CI
  enforces both.
- TypeScript: Biome (`bun run lint`) for formatting and linting, and `tsc` in strict mode with
  `noUncheckedIndexedAccess` (`bun run typecheck`). CI enforces both.
- Every file the project writes starts with the licence header in its own comment syntax
  (`make check-headers`). Files that cannot carry one are listed in `scripts/check-headers.ts`.

### Comments describe how the code *is*

A comment explains what the code does and why it is that way. It does not narrate what the code
used to be or what was tried. Where the reason for a choice is that the alternative is worse, say
so in the present tense: "a 1600-pixel JPEG is around 300 KB, so the cap is 2 MB", not "photos
used to fill the database". `git log` and `git blame` hold the history.

## Testing

Run the tests often while coding: `make test` for the unit and API tests (seconds), `make e2e`
for the end-to-end tests (a minute), `make integration-test` for the container (a few minutes).

**Each test in `src/` must run in under 300 ms. Each test in `tests/` must run in under 1
second.** The budgets are enforced: `cargo nextest run` reports a test past its budget as SLOW,
per `.config/nextest.toml`. The container tests have 60 seconds and two retries, because they
race real services; nothing else retries, because a flake anywhere else is a bug.

### Unit tests

Each file in `src/` ends with its own test module, covering both the happy path and the corner
cases. `src/testing.rs` (`Home`) gives them a household with people and cards in a real database
in memory. The web app's pure modules have their tests beside them (`web/src/*.test.ts`, run by
`bun test`): the barcode checks and drawing, the offline mirror, the API client.

### End-to-end tests

- `tests/api.rs` sends requests through the real router and every guard in front of it
  (`tower::ServiceExt::oneshot`, no socket), as the app sends them: sign-in and its throttle,
  CSRF refusals, the household boundary, private cards, photos, invites.
- `tests/cli.rs` runs the real binary: `serve` and `healthz`, SIGTERM, the administrator's
  commands.
- `tests/upgrade.rs` opens a database written by every release (see "Releasing").
- `web/e2e/` runs the real binary with the built app in Chromium and WebKit at a phone's size,
  driven by Playwright from `bun test`: joining by invite, adding cards, the till view, sharing
  and private cards, the app offline. `camera.e2e.ts` feeds Chromium's fake camera a video of a
  barcode and checks the scan fills the card in. `bun run e2e camera` runs one file;
  `run.ts` explains the rest. WebKit needs system libraries; where they are missing the flows run
  in Chromium only and the runner says so.
- `tests/container.rs` builds the image, runs it read-only behind a real `nginx:alpine` in
  podman or docker, and signs in through the proxy. It runs only with
  `LOYALTY_CONTAINER_TESTS=1` (`make integration-test`).

### Golden files

Generated output is pinned byte for byte: the schema, `--help`, the API's JSON
(`tests/golden/`) and drawn barcodes (`web/src/golden/`). After an intended change, regenerate
with `UPDATE_GOLDENS=1 cargo nextest run` or `UPDATE_GOLDENS=1 bun test src`, and review the
diff like any other.

### Coverage

96.4% of the Rust lines (2,763 lines, 99 not covered), measured on 2026-10-16 with
`cargo llvm-cov nextest --summary-only`. This is the one place that figure is quoted. It
understates coverage: the end-to-end and container tests run the binary in another process, so
their coverage never comes back.

CI fails under **90%**, which is the promise; the figure above is a snapshot. A floor at
today's figure would be a trap: with no headroom, one ordinary function landing slightly
under-tested turns CI red on an unrelated change, and the quickest fix is to lower the floor,
which is the rot it exists to prevent. Raising the floor as the figure rises is welcome; it moves
together with this paragraph.

The web app has no coverage gate: `bun test` counts only the files a test loads, so its figure
says little, and the e2e tests are what cover the screens.

### How should tests handle dependencies?

**No mocks.** Mocks test the mock of an interface, not the interface, and they break when the
code is improved. Use the real implementation. Where it can't be, in order of preference:

- **In-memory fakes** for storage: SQLite's in-memory database (`Db::open_in_memory`), tempdirs,
  `MemoryKV` for the app's IndexedDB.
- **Real product inputs** rather than test back-doors: `--listen 127.0.0.1:0`, `--db`, `--origin`.
- **A fake server** for the web app's API client (`web/src/fake.ts`), which answers like the
  real one; the real one is tested against the app end to end.

**No test touches the network.** The e2e tests talk to a server on the loopback interface; the
barcode tests load zxing's WebAssembly from `node_modules`, never from its CDN.

## Releasing

1. `make ci` and `make integration-test` pass.
2. The version in `Cargo.toml` and `web/package.json` is the new one, and `CHANGELOG.md` has a
   dated section for it: `bun run scripts/check-versions.ts --release`.
3. Record the release's database for the upgrade test: run the release binary on a new
   database, add a household, two people, a family card, a private card and a photo, dump it
   with `sqlite3 loyalty.db .dump > tests/fixtures/db/db-<version>.sql`, and add the version to
   `RELEASES` in `tests/upgrade.rs`.
4. Tag `v<version>` on `main`.

## Code structure

```
<root of the repository>
    |- /src                 <- The server
        |- main.rs          <- Entry point: parses the command line and runs it
        |- lib.rs           <- Everything main.rs runs, reachable from tests/
        |- cli.rs           <- The commands: serve, healthz, household, invite, user, backup
        |- db.rs            <- The database file; db/schema.rs, its migrations
        |- accounts.rs      <- Households, people, passwords, sessions, invites
        |- cards.rs         <- Cards, who may see them, and the barcode checks
        |- photos.rs        <- Front and back photos, stored by content
        |- throttle.rs      <- Slowing down password guessing
        |- web.rs           <- The router's guards: headers, CSRF, sessions, the request log
        |- web/api.rs       <- The routes and their handlers
        |- web/serve.rs     <- The accept loop
        |- assets.rs        <- The embedded web app
        |- testing.rs, golden.rs <- Test fixtures and golden files
    |- /tests               <- End-to-end tests of the server, the binary and the image
    |- /web                 <- The web app
        |- /src             <- Its code, with its unit tests
        |- /e2e             <- The end-to-end tests in a browser
        |- /public          <- The manifest and icons, as served
        |- build.ts         <- Builds web/dist
    |- /packaging           <- The Dockerfile, the Compose service, the nginx site, and how to
                               run it (README.md); the third-party notices' configuration
    |- /scripts             <- Developer scripts, run with Bun
    |- README.md            <- What it is. Readable by anyone.
    |- CONTRIBUTING.md      <- This file
    |- AGENTS.md            <- AI-only instructions
    |- REVIEW.md            <- Open code-health items
```

## Makefile targets

`make help` lists them. `make ci` runs what CI runs, except the container tests
(`make integration-test`).

# Loyalty Card App

A household's loyalty cards, on its own server. One app for every phone in the family, Android or
iPhone, installed from the browser: no app store, no account with anyone, and nothing leaves your
server.

- **Shared the way a family shares.** The supermarket card is the family's; the flight miles are
  each person's own. Everyone in the household sees every card, labelled with whose it is, so a
  parent can show a child's card at the till. A card marked private is its holder's alone.
- **Scanned, not typed.** Point the camera at the card and the code is read: EAN, UPC, Code 39,
  93 and 128, ITF, Codabar, QR, PDF417 (boarding passes), Aztec and Data Matrix. Or take a photo of
  the card, or type the number in, with the check digit checked so a typing mistake shows now and
  not at the till.
- **Clean at the till.** The card is shown as its code, drawn fresh: black on white, sharp at any
  size, the number under it, the screen kept on. The photos of the front and back are one tap
  away.
- **Works with no signal.** The app and your cards are kept on the phone, so they open in a shop's
  basement.

## Run it

You need a server with Docker, a name for the app with a TLS certificate, and nginx in front of
it: a phone only lets a web app use the camera over HTTPS. The image is published for amd64 and
arm64, so a Raspberry Pi runs it too:

    ghcr.io/ivankovic/loyalty-card-app:0.1

Add [the Compose service](packaging/compose/loyalty.yml) and [the nginx site](packaging/nginx/loyalty.conf),
start it, and make your household and your own invite:

    docker exec loyalty /loyalty household add Home
    docker exec loyalty /loyalty invite --household Home alice --admin

Open the printed link on your phone, choose a password, and add the app to the home screen.
[packaging/README.md](packaging/README.md) is the whole guide: the data folder, nginx on the host
or in Compose, updating and backups.

## How it is built

A Rust server (axum, SQLite) with the web app built into its binary, as one small container.
The web app is TypeScript, built with Bun; zxing-cpp, compiled to WebAssembly, reads and draws
the codes. See [packaging/README.md](packaging/README.md) to run it, and
[CONTRIBUTING.md](CONTRIBUTING.md) for how it is made and tested.

One thing a web app cannot do is turn the screen's brightness up, so the app asks you to. A thin
native wrapper that can is on the list.

# License

Copyright (C) 2026 Marko Ivankovic

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU Affero General Public License as published
by the Free Software Foundation, version 3 of the License.

See the LICENSE file for the full text of the License. The licenses of what it is built from are
in [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md). Files that cannot carry a license header
(JSON, images, lock files, the test fixtures) are under the same license.

## Cannot use AGPL software?

A commercial license is available as a monthly subscription through
[GitHub Sponsors](https://github.com/sponsors/ivankovic). It covers internal use of Loyalty Card
App by your organisation without the source-disclosure obligations of the AGPL. The terms are in
[LICENSE-COMMERCIAL](LICENSE-COMMERCIAL). Pick the tier that names the commercial license as a
benefit. For invoicing or other arrangements, contact me at
[marko@ivankovic.me](mailto:marko@ivankovic.me).

# AI policy

This project uses substantial AI assistance, currently Claude Code. Most commits disclose this
with a `Co-Authored-By` trailer and a link to the session that produced them. This project does not
hide that fact. This project does not treat AI assistance as a lesser way to write software.
AI-assisted contributions are as welcome as any other, disclosed the same way: whoever submits the
work is responsible for understanding it and standing behind it.

# For Developers, human or otherwise

See [CONTRIBUTING.md](CONTRIBUTING.md) for the technology, the code-quality and testing
expectations, the project's structure, and what CI checks on every push and pull request.
Contributions are welcome under the [Contributor License Agreement](CLA.md). `AGENTS.md` has
additional AI-agent-specific conventions.

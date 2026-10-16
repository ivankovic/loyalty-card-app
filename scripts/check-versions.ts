// This file is part of Loyalty Card App.
//
// Copyright (C) 2026 Marko Ivankovic
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, version 3 of the License.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

// Fails when the version is not the same everywhere it is written: Cargo.toml (the binary's
// --version), web/package.json (shown in the app's settings), and, with --release, a dated
// section in CHANGELOG.md.

import { join } from "node:path";

const root = join(import.meta.dir, "..");
const cargo = /^version = "([^"]+)"/m.exec(await Bun.file(join(root, "Cargo.toml")).text())?.[1];
const web = (await Bun.file(join(root, "web/package.json")).json()).version;
const problems: string[] = [];
if (!cargo) problems.push("Cargo.toml has no version");
if (cargo !== web) problems.push(`Cargo.toml says ${cargo}, web/package.json says ${web}`);
if (process.argv.includes("--release")) {
  const changelog = await Bun.file(join(root, "CHANGELOG.md")).text();
  if (!new RegExp(`^## \\[${cargo?.replaceAll(".", "\\.")}\\] - \\d{4}-\\d{2}-\\d{2}$`, "m").test(changelog)) {
    problems.push(`CHANGELOG.md has no dated section for ${cargo}`);
  }
}
if (problems.length > 0) {
  console.error(problems.join("\n"));
  process.exit(1);
}
console.log(`version ${cargo} everywhere`);

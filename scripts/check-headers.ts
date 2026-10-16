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

// Fails when a file in the repository lacks the licence header: every file the project wrote,
// in its own comment syntax. Files that cannot carry a comment (JSON, images, lock files, the
// goldens, the licence texts themselves) are listed below and covered by the README's licence
// section instead.

import { join } from "node:path";

const root = join(import.meta.dir, "..");
const exempt = [
  /^LICENSE/,
  /^THIRD-PARTY-NOTICES\.md$/,
  /\.(md|json|lock|png|svg|webmanifest|y4m|toml\.lock)$/,
  /^Cargo\.lock$/,
  /^web\/bun\.lock$/,
  /^tests\/golden\//,
  /^web\/src\/golden\//,
  /^tests\/fixtures\//,
  /(^|\/)\.gitkeep$/,
  /^\.gitignore$|^web\/\.gitignore$|^\.dockerignore$/,
  /^packaging\/notices\/third-party-notices\.hbs$/,
];
const files = (await Bun.$`git ls-files --cached --others --exclude-standard`.cwd(root).text())
  .split("\n")
  .filter(Boolean)
  .filter((f) => !exempt.some((re) => re.test(f)));
const missing: string[] = [];
for (const file of files) {
  const head = (await Bun.file(join(root, file)).text()).slice(0, 600);
  if (!head.includes("This file is part of Loyalty Card App.") || !head.includes("GNU Affero General Public License")) {
    missing.push(file);
  }
}
if (missing.length > 0) {
  console.error(`These files have no licence header:\n${missing.join("\n")}`);
  process.exit(1);
}
console.log(`${files.length} files carry the licence header`);

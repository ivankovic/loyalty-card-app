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

// Runs the end-to-end tests: builds the app and the server once, then each *.e2e.ts file in a
// process of its own (a browser a file starts must not outlive it into the next one).
//
//   bun run e2e                     every file
//   bun run e2e camera              the files whose names contain "camera"
//   E2E_SHARD=1/3 bun run e2e       one third of the files, for CI
//
// WebKit runs where the machine has its libraries (CI does); elsewhere the flows run in
// Chromium only, and this says so at the end.

import { readdir } from "node:fs/promises";
import { join } from "node:path";
import { webkit } from "playwright";

const here = import.meta.dir;
const root = join(here, "../..");

async function run(cmd: string[], cwd: string): Promise<void> {
  const proc = Bun.spawn(cmd, { cwd, stdout: "inherit", stderr: "inherit" });
  if ((await proc.exited) !== 0) throw new Error(`${cmd.join(" ")} failed`);
}

if (!process.env.E2E_PREBUILT) {
  await run(["bun", "run", "build"], join(here, ".."));
  await run(["cargo", "build", "--locked", "--bin", "loyalty"], root);
}

let browsers = process.env.E2E_BROWSERS;
let webkitMissing = false;
if (!browsers) {
  try {
    await (await webkit.launch()).close();
    browsers = "chromium,webkit";
  } catch {
    browsers = "chromium";
    webkitMissing = true;
  }
}

const filter = process.argv[2] ?? "";
let files = (await readdir(here)).filter((f) => f.endsWith(".e2e.ts") && f.includes(filter)).sort();
const shard = process.env.E2E_SHARD;
if (shard) {
  const [n, m] = shard.split("/").map(Number);
  if (!n || !m) throw new Error(`E2E_SHARD is n/m, not ${shard}`);
  files = files.filter((_, i) => i % m === n - 1);
}

const failed: string[] = [];
for (const file of files) {
  const proc = Bun.spawn(["bun", "test", "--timeout", "60000", `./${file}`], {
    cwd: here,
    env: { ...process.env, E2E_BROWSERS: browsers },
    stdout: "inherit",
    stderr: "inherit",
  });
  if ((await proc.exited) !== 0) failed.push(file);
}

if (webkitMissing) {
  console.warn(
    "\nWebKit could not start on this machine, so the flows ran in Chromium only.\n" +
      "Install its libraries with: sudo bun node_modules/playwright/cli.js install-deps webkit\n",
  );
}
if (failed.length > 0) {
  console.error(`\nFailed: ${failed.join(", ")}`);
  process.exit(1);
}
console.log(`\n${files.length} files passed (${browsers}).`);

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

// The notices for what the web app ships from npm: the packages in web/package.json's
// `dependencies` (not the tools in `devDependencies`, which never reach a phone), with their
// licence texts. `make third-party-notices` appends this to cargo-about's part.

import { join } from "node:path";

const web = join(import.meta.dir, "../web");
const pkg = await Bun.file(join(web, "package.json")).json();
const names = Object.keys(pkg.dependencies ?? {}).sort();

let out = "\n# The web app\n\n";
out +=
  "The web app that the binary serves is built from these npm packages, each under its own\n" +
  "license.\n\n";
for (const name of names) {
  const dir = join(web, "node_modules", name);
  const meta = await Bun.file(join(dir, "package.json")).json();
  const license = await Bun.file(join(dir, "LICENSE")).text();
  out += `## ${name} ${meta.version} (${meta.license})\n\n`;
  if (name === "zxing-wasm") {
    out +=
      "Its WebAssembly is built from zxing-cpp (https://github.com/zxing-cpp/zxing-cpp),\n" +
      "Copyright the ZXing authors, under the Apache License 2.0, and from zint\n" +
      "(https://github.com/zint/zint), Copyright Robin Stuart and the zint contributors, under\n" +
      'the BSD 3-Clause "New" or "Revised" License. The full texts of both licenses are above.\n\n';
  }
  out += `\`\`\`\n${license.trim()}\n\`\`\`\n\n`;
}
process.stdout.write(out);

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

// Builds the app into dist/, which the Rust binary embeds (see build.rs there).
//
//   dist/index.html             the page, pointing at the hashed files
//   dist/assets/main-<hash>.js  the app, bundled and minified; its CSS beside it
//   dist/assets/zxing_full-<version>.wasm
//   dist/sw.js                  the service worker, with the list of files to keep offline
//   dist/*                      public/: the manifest and the icons
//
// Names under assets/ change with their content, so the server lets browsers keep them for a
// year; the rest keep their names and are revalidated, which is how a release reaches a phone.

import { cp, mkdir, readdir, rm } from "node:fs/promises";
import { basename, join } from "node:path";
import zxingPkg from "./node_modules/zxing-wasm/package.json";
import pkg from "./package.json";

const root = import.meta.dir;
const dist = join(root, "dist");
const wasmName = `zxing_full-${zxingPkg.version}.wasm`;

await rm(dist, { recursive: true, force: true });
await mkdir(join(dist, "assets"), { recursive: true });

const app = await Bun.build({
  entrypoints: [join(root, "src/main.ts")],
  outdir: join(dist, "assets"),
  naming: "[name]-[hash].[ext]",
  target: "browser",
  minify: true,
  define: {
    WASM_URL: JSON.stringify(`/assets/${wasmName}`),
    APP_VERSION: JSON.stringify(pkg.version),
  },
});
if (!app.success) {
  for (const log of app.logs) console.error(log);
  process.exit(1);
}
const outputs = app.outputs.map((o) => `/assets/${basename(o.path)}`);
const js = outputs.find((p) => p.endsWith(".js"));
const css = outputs.find((p) => p.endsWith(".css"));
if (!js || !css)
  throw new Error(`the bundle is missing its script or styles: ${outputs.join(", ")}`);

await cp(
  join(root, "node_modules/zxing-wasm/dist/full/zxing_full.wasm"),
  join(dist, "assets", wasmName),
);
await cp(join(root, "public"), dist, { recursive: true });

const page = (await Bun.file(join(root, "src/index.html")).text())
  .replace("{{CSS}}", css)
  .replace("{{JS}}", js);
await Bun.write(join(dist, "index.html"), page);

const kept = [
  "/index.html",
  ...outputs,
  `/assets/${wasmName}`,
  ...(await readdir(join(root, "public"))).map((f) => `/${f}`),
].sort();
// The cache's name changes whenever a kept file does, which is what makes a phone take the
// new release and drop the old one.
const version = new Bun.CryptoHasher("sha256").update(kept.join("\n")).digest("hex").slice(0, 16);
const worker = await Bun.build({
  entrypoints: [join(root, "src/sw.ts")],
  target: "browser",
  minify: true,
  define: {
    PRECACHE: JSON.stringify(kept),
    CACHE: JSON.stringify(`app-${version}`),
  },
});
if (!worker.success || !worker.outputs[0]) {
  for (const log of worker.logs) console.error(log);
  process.exit(1);
}
await Bun.write(join(dist, "sw.js"), await worker.outputs[0].text());

console.log(`built dist/ (${kept.length} files kept offline, cache app-${version})`);

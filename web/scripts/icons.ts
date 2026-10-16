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

// Draws the PNG icons from public/icon.svg, for the phones that want PNGs (an iPhone's home
// screen, Android's install prompt). Run after changing the SVG, then commit both:
//
//     bun run icons
//
// Uses the e2e tests' Chromium; nothing else here needs a rasteriser.

import { join } from "node:path";
import { chromium } from "playwright";

const pub = join(import.meta.dir, "../public");
const svg = await Bun.file(join(pub, "icon.svg")).text();
const browser = await chromium.launch();
try {
  for (const [name, size] of [
    ["icon-192.png", 192],
    ["icon-512.png", 512],
    ["apple-touch-icon.png", 180],
  ] as const) {
    const page = await browser.newPage({ viewport: { width: size, height: size } });
    await page.setContent(
      `<style>html,body{margin:0}svg{display:block;width:${size}px;height:${size}px}</style>${svg}`,
    );
    await page.screenshot({ path: join(pub, name), omitBackground: false });
    await page.close();
    console.log(`wrote public/${name}`);
  }
} finally {
  await browser.close();
}

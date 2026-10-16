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

// Golden files for the web app's tests, as on the Rust side: an exact copy of a generated text in
// src/golden/, compared byte for byte. UPDATE_GOLDENS=1 bun test rewrites them; review the diff.

import { expect } from "bun:test";

export async function assertGolden(name: string, actual: string): Promise<void> {
  const file = Bun.file(new URL(`./golden/${name}`, import.meta.url).pathname);
  if (process.env.UPDATE_GOLDENS) {
    await Bun.write(file, actual);
    return;
  }
  if (!(await file.exists())) {
    throw new Error(
      `missing golden src/golden/${name}: run UPDATE_GOLDENS=1 bun test, review it, commit it`,
    );
  }
  expect(actual).toBe(await file.text());
}

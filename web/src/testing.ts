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

// What the unit tests share: zxing with its WebAssembly loaded from node_modules.

import { configure } from "./zxing";

let ready = false;

export async function zxingForTests(): Promise<void> {
  if (ready) return;
  const wasm = Bun.file(
    new URL("../node_modules/zxing-wasm/dist/full/zxing_full.wasm", import.meta.url).pathname,
  );
  configure({ wasmBinary: await wasm.arrayBuffer() });
  ready = true;
}

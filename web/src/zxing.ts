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

// zxing-wasm, configured once: where its WebAssembly comes from. The app serves the file from
// its own origin (never the library's default CDN); the tests hand it the bytes.

import { prepareZXingModule, readBarcodes, writeBarcode } from "zxing-wasm/full";

export type Overrides = { locateFile: (path: string) => string } | { wasmBinary: ArrayBuffer };

export function configure(overrides: Overrides): void {
  prepareZXingModule({ overrides, fireImmediately: false });
}

export { readBarcodes, writeBarcode };

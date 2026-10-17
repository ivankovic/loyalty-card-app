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

// The app's entry point.

import "./style.css";
import { App } from "./app";
import { applyTheme } from "./theme";
import { configure } from "./zxing";

applyTheme();
configure({ locateFile: () => WASM_URL });

const root = document.getElementById("app");
if (root) void new App(root).start();

if ("serviceWorker" in navigator) {
  navigator.serviceWorker.register("/sw.js").catch(() => {
    // Without it the app still works online; it just will not open offline.
  });
}

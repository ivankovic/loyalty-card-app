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

import { expect, test } from "bun:test";
import { fitWithin, MAX_SIDE } from "./photo";

test("a large photo is scaled down to fit, keeping its shape", () => {
  expect(fitWithin(4032, 3024, MAX_SIDE)).toEqual({ width: 1600, height: 1200 });
  expect(fitWithin(3024, 4032, MAX_SIDE)).toEqual({ width: 1200, height: 1600 });
});

test("a small photo is left as it is", () => {
  expect(fitWithin(800, 600, MAX_SIDE)).toEqual({ width: 800, height: 600 });
});

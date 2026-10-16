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

// The household: family cards for everyone, a private card for its holder alone.

import { describe, expect, test } from "bun:test";
import { addCard, arrive, BROWSERS, phone, Server } from "./harness";

for (const kind of BROWSERS) {
  describe(kind, () => {
    test("everyone sees the family's cards and nobody else sees a private one", async () => {
      const server = await Server.start();
      const marko = await phone(server, kind);
      await arrive(marko.page, await server.invite("marko", true), "Marko");
      const ana = await phone(server, kind);
      await arrive(ana.page, await server.invite("ana"), "Ana");

      await addCard(marko.page, { name: "Coop", code: "400638133393" });
      await addCard(ana.page, {
        name: "Diary club",
        code: "D-1",
        kind: "Code 128",
        holder: "Ana",
        private: true,
      });
      await addCard(ana.page, { name: "Library", code: "L-2", kind: "Code 128", holder: "Ana" });

      await marko.page.goto("/#/");
      await marko.page.reload();
      await marko.page.getByText("Library").waitFor();
      const markoSees = await marko.page.locator(".tile-name").allInnerTexts();
      expect(markoSees.sort()).toEqual(["Coop", "Library"]);

      await ana.page.goto("/#/");
      await ana.page.reload();
      await ana.page.getByText("Coop").waitFor();
      const anaSees = await ana.page.locator(".tile-name").allInnerTexts();
      expect(anaSees.sort()).toEqual(["Coop", "Diary club", "Library"]);
    });
  });
}

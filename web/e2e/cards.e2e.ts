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

// A card's life: added by typing, shown at the till, changed, deleted.

import { describe, expect, test } from "bun:test";
import { addCard, arrive, BROWSERS, phone, Server } from "./harness";

for (const kind of BROWSERS) {
  describe(kind, () => {
    test("a typed card is shown at the till with its check digit", async () => {
      const server = await Server.start();
      const { page, errors } = await phone(server, kind);
      await arrive(page, await server.invite("marko", true));
      await addCard(page, { name: "Coop Supercard", code: "400638133393" });
      expect(await page.getByRole("heading", { name: "Coop Supercard" }).isVisible()).toBe(true);
      expect(await page.getByText("4006381333931").isVisible()).toBe(true);
      const label = await page.locator("svg.barcode").getAttribute("aria-label");
      expect(label).toBe("EAN-13 400638133393");
      expect(errors).toEqual([]);
    });

    test("a typing mistake in the number is caught before saving", async () => {
      const server = await Server.start();
      const { page } = await phone(server, kind);
      await arrive(page, await server.invite("marko", true));
      await page.getByRole("link", { name: "+ Add a card" }).click();
      await page.getByRole("button", { name: "Type it in" }).click();
      await page.getByLabel("Name", { exact: true }).fill("Coop");
      await page.getByLabel("Code", { exact: true }).fill("4006381333932");
      await page.getByText("the last digit should be 1: the number has a typing mistake").waitFor();
      await page.getByRole("button", { name: "Add the card" }).click();
      expect(page.url()).toEndWith("#/new");
    });

    test("a card is changed and deleted", async () => {
      const server = await Server.start();
      const { page } = await phone(server, kind);
      await arrive(page, await server.invite("marko", true));
      await addCard(page, { name: "Migros", code: "M-12345", kind: "Code 128" });
      await page.getByRole("link", { name: "Edit" }).click();
      await page.getByLabel("Name", { exact: true }).fill("Migros Cumulus");
      await page.getByRole("button", { name: "Save" }).click();
      await page.getByRole("heading", { name: "Migros Cumulus" }).waitFor();
      await page.getByRole("link", { name: "Edit" }).click();
      page.once("dialog", (d) => d.accept());
      await page.getByRole("button", { name: "Delete the card" }).click();
      await page.getByText("No cards yet. Add the first one.").waitFor();
    });

    test("the list is searched and filtered by holder", async () => {
      const server = await Server.start();
      const { page } = await phone(server, kind);
      await arrive(page, await server.invite("marko", true), "Marko");
      await addCard(page, { name: "Coop", code: "400638133393" });
      await addCard(page, { name: "Miles", code: "992000123", kind: "Code 128", holder: "Marko" });
      await page.goto("/#/");
      await page.getByRole("radio", { name: "Family" }).click();
      expect(await page.locator(".tile").allInnerTexts()).toEqual([
        expect.stringContaining("Coop"),
      ]);
      await page.getByRole("radio", { name: "All" }).click();
      await page.getByRole("searchbox").fill("mil");
      expect(await page.locator(".tile").allInnerTexts()).toEqual([
        expect.stringContaining("Miles"),
      ]);
    });
  });
}

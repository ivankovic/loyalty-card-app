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

// At a till with no signal: the app opens and shows the cards it saw last.

import { describe, expect, test } from "bun:test";
import { addCard, arrive, BROWSERS, phone, Server } from "./harness";

// Chromium only: Playwright's WebKit cannot reload a page while offline ("WebKit encountered an
// internal error"), so the service worker is never asked. On an iPhone this is checked by hand
// (CONTRIBUTING.md, "Releasing").
for (const kind of BROWSERS.filter((b) => b === "chromium")) {
  describe(kind, () => {
    test("the cards open and draw with the phone offline", async () => {
      const server = await Server.start();
      const { context, page } = await phone(server, kind);
      await arrive(page, await server.invite("marko", true));
      await addCard(page, { name: "Coop", code: "400638133393" });
      await page.goto("/#/");
      // The service worker has taken the app in once it controls the page.
      await page.waitForFunction(() => navigator.serviceWorker.controller !== null, null, {
        timeout: 10_000,
      });

      await context.setOffline(true);
      await page.reload();
      await page.getByText(/^Offline: these are the cards as of/).waitFor();
      await page.getByText("Coop").click();
      await page.locator("svg.barcode").waitFor();
      expect(await page.getByText("4006381333931").isVisible()).toBe(true);
      expect(await page.getByRole("link", { name: "+ Add a card" }).count()).toBe(0);
      await context.setOffline(false);
    });
  });
}

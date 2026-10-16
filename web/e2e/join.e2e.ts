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

// Arriving: an invite link, a password, and signing in again later.

import { describe, expect, test } from "bun:test";
import { arrive, BROWSERS, PASSWORD, phone, Server } from "./harness";

for (const kind of BROWSERS) {
  describe(kind, () => {
    test("someone invited joins, signs out and signs back in", async () => {
      const server = await Server.start();
      const { page, errors } = await phone(server, kind);
      await arrive(page, await server.invite("marko", true), "Marko");
      expect(await page.getByRole("heading", { name: "Home" }).isVisible()).toBe(true);
      expect(await page.getByText("No cards yet. Add the first one.").isVisible()).toBe(true);

      await page.getByRole("link", { name: "Settings" }).click();
      await page.getByRole("heading", { name: "Marko" }).waitFor();
      await page.getByRole("button", { name: "Sign out" }).click();
      await page.getByRole("button", { name: "Sign in" }).waitFor();

      await page.getByLabel("Name").fill("marko");
      await page.getByLabel("Password").fill("not the password");
      await page.getByRole("button", { name: "Sign in" }).click();
      await page.getByText("Wrong name or password").waitFor();
      await page.getByLabel("Password").fill(PASSWORD);
      await page.getByRole("button", { name: "Sign in" }).click();
      await page.getByRole("heading", { name: "Home" }).waitFor();
      expect(errors.filter((e) => !e.includes("401"))).toEqual([]);
    });

    test("an invite link works once", async () => {
      const server = await Server.start();
      const link = await server.invite("ana");
      const first = await phone(server, kind);
      await arrive(first.page, link);
      const second = await phone(server, kind);
      await second.page.goto(link);
      await second.page.getByRole("heading", { name: "This link does not work" }).waitFor();
    });

    test("an invite link cut short says so rather than asking to sign in", async () => {
      const server = await Server.start();
      const link = await server.invite("ana");
      const { page } = await phone(server, kind);
      await page.goto(link.slice(0, -20));
      await page.getByRole("heading", { name: "This link does not work" }).waitFor();
      await page.getByText("The link is incomplete").waitFor();
      expect(await page.getByRole("button", { name: "Sign in" }).count()).toBe(0);
    });

    test("an administrator makes an invite link in the app", async () => {
      const server = await Server.start();
      const { page } = await phone(server, kind);
      await arrive(page, await server.invite("marko", true));
      await page.goto("/#/settings");
      await page.getByLabel("The name they sign in with").fill("luka");
      await page.getByRole("button", { name: "Make an invite link" }).click();
      const link = await page.locator("input.link").inputValue();
      expect(link).toMatch(new RegExp(`^${server.origin}/i/[0-9a-f]{64}$`));
    });
  });
}

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

// Scanning: a code in front of the camera fills the card in; a photo of the card does too.
// Chromium only: its fake camera plays a video file, which no other browser offers.

import { expect, test } from "bun:test";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { arrive, cameraFile, codeImage, phone, Server } from "./harness";

test("a code held up to the camera is read into a new card", async () => {
  const server = await Server.start();
  const video = await cameraFile(await codeImage("EAN13", "400638133393"));
  const { page, errors } = await phone(server, "chromium", {
    launch: {
      args: [
        "--use-fake-ui-for-media-stream",
        "--use-fake-device-for-media-stream",
        `--use-file-for-fake-video-capture=${video}`,
      ],
    },
    context: { permissions: ["camera"] },
  });
  await arrive(page, await server.invite("marko", true));
  await page.getByRole("link", { name: "+ Add a card" }).click();
  const scanner = page.getByRole("dialog", { name: "Scan the code" });
  await scanner.waitFor();
  await scanner.waitFor({ state: "detached", timeout: 15_000 });
  expect(await page.getByLabel("Code", { exact: true }).inputValue()).toBe("4006381333931");
  expect(await page.getByLabel("Kind").inputValue()).toBe("ean13");
  expect(errors).toEqual([]);
});

test("a photo of the card's back fills in its code", async () => {
  const server = await Server.start();
  const { page } = await phone(server, "chromium");
  await arrive(page, await server.invite("marko", true));
  await page.getByRole("link", { name: "+ Add a card" }).click();
  await page.getByRole("button", { name: "Type it in" }).click();
  const photo = join(tmpdir(), `loyalty-photo-${process.pid}.png`);
  await Bun.write(photo, await codeImage("QRCode", "MEMBER 0042", 8));
  const chooser = page.waitForEvent("filechooser");
  await page.locator(".photo-slot").nth(1).getByRole("button", { name: "Add a photo" }).click();
  await (await chooser).setFiles(photo);
  await page.getByAltText("Back (not saved yet)").waitFor();
  expect(await page.getByLabel("Code", { exact: true }).inputValue()).toBe("MEMBER 0042");
  expect(await page.getByLabel("Kind").inputValue()).toBe("qr");
  await page.getByLabel("Name", { exact: true }).fill("Gym");
  await page.getByRole("button", { name: "Add the card" }).click();
  await page.getByRole("button", { name: "Back" }).click();
  const img = page.getByAltText("The card's back");
  await img.waitFor();
  expect(await img.evaluate((i: HTMLImageElement) => i.naturalWidth)).toBeGreaterThan(0);
});

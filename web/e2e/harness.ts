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

// The end-to-end harness: the real server binary on a database of its own, the built app it
// embeds, and a browser driven by Playwright, at a phone's size.
//
// Tests are written against what a person sees (labels, button names, text), not against the
// page's structure, so that they read as the steps someone takes.

import { afterAll } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  type Browser,
  type BrowserContext,
  type BrowserContextOptions,
  chromium,
  type LaunchOptions,
  type Page,
  webkit,
} from "playwright";

export const ROOT = join(import.meta.dir, "../..");
export const BINARY = join(ROOT, "target/debug/loyalty");
export const PASSWORD = "a good password";

/** The browsers the flows run in: E2E_BROWSERS, or both. run.ts sets it to what this machine has. */
export const BROWSERS = (process.env.E2E_BROWSERS ?? "chromium,webkit")
  .split(",")
  .filter(Boolean) as ("chromium" | "webkit")[];

const cleanups: (() => Promise<void>)[] = [];
afterAll(async () => {
  for (const cleanup of cleanups.reverse()) await cleanup();
});

/** A free port on the loopback interface. */
function freePort(): number {
  const server = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const port = server.port;
  server.stop(true);
  return port;
}

export class Server {
  private constructor(
    readonly origin: string,
    private readonly db: string,
  ) {}

  /** A server with one household, "Home", and nobody in it yet. */
  static async start(): Promise<Server> {
    const dir = await mkdtemp(join(tmpdir(), "loyalty-e2e-"));
    const db = join(dir, "loyalty.db");
    const origin = `http://127.0.0.1:${freePort()}`;
    const server = new Server(origin, db);
    await server.cli("household", "add", "Home");
    const process = Bun.spawn(
      [BINARY, "--db", db, "serve", "--listen", origin.slice("http://".length), "--origin", origin],
      { stderr: "pipe", stdout: "ignore" },
    );
    cleanups.push(async () => {
      process.kill();
      await process.exited;
      await rm(dir, { recursive: true, force: true });
    });
    for (let i = 0; i < 100; i++) {
      const ok = await fetch(`${origin}/healthz`).then(
        (r) => r.ok,
        () => false,
      );
      if (ok) return server;
      await Bun.sleep(50);
    }
    throw new Error(`the server never answered: ${await new Response(process.stderr).text()}`);
  }

  async cli(...args: string[]): Promise<string> {
    const run = Bun.spawn([BINARY, "--db", this.db, ...args], {
      env: { ...process.env, LOYALTY_ORIGIN: this.origin },
      stdout: "pipe",
      stderr: "pipe",
    });
    const [out, err, code] = await Promise.all([
      new Response(run.stdout).text(),
      new Response(run.stderr).text(),
      run.exited,
    ]);
    if (code !== 0) throw new Error(`loyalty ${args.join(" ")} failed: ${err}`);
    return out;
  }

  /** The invite link for `name`, as an administrator would make it. */
  async invite(name: string, admin = false): Promise<string> {
    const out = await this.cli(
      "invite",
      "--household",
      "Home",
      name,
      ...(admin ? ["--admin"] : []),
    );
    const link = /https?:\/\/\S+\/i\/[0-9a-f]{64}/.exec(out)?.[0];
    if (!link) throw new Error(`no link in: ${out}`);
    return link;
  }
}

export interface Phone {
  context: BrowserContext;
  page: Page;
  /** What the page logged as errors, and every page error: a test ends by expecting none. */
  errors: string[];
}

const browsers = new Map<string, Promise<Browser>>();

function browser(kind: "chromium" | "webkit", launch: LaunchOptions = {}): Promise<Browser> {
  const key = `${kind} ${JSON.stringify(launch)}`;
  let found = browsers.get(key);
  if (!found) {
    found = (kind === "chromium" ? chromium : webkit).launch(launch);
    browsers.set(key, found);
    cleanups.push(async () => (await found)?.close());
  }
  return found;
}

/** A phone-sized browser on the app. */
export async function phone(
  server: Server,
  kind: "chromium" | "webkit" = "chromium",
  options: { launch?: LaunchOptions; context?: BrowserContextOptions } = {},
): Promise<Phone> {
  const context = await (await browser(kind, options.launch)).newContext({
    viewport: { width: 390, height: 844 },
    deviceScaleFactor: 2,
    isMobile: kind === "chromium",
    hasTouch: true,
    baseURL: server.origin,
    ...options.context,
  });
  cleanups.push(() => context.close());
  const page = await context.newPage();
  const errors: string[] = [];
  page.on("console", (m) => {
    if (m.type() === "error") errors.push(m.text());
  });
  page.on("pageerror", (e) => errors.push(e.message));
  return { context, page, errors };
}

/** Opens an invite link and chooses the password: the first thing anyone does. */
export async function arrive(page: Page, link: string, displayName?: string): Promise<void> {
  await page.goto(link);
  if (displayName) await page.getByLabel("Your name, as the others see it").fill(displayName);
  await page.getByLabel("Choose a password").fill(PASSWORD);
  await page.getByLabel("The password again").fill(PASSWORD);
  await page.getByRole("button", { name: "Join" }).click();
  await page.getByRole("searchbox", { name: "Search the cards" }).waitFor();
}

/** Adds a card by typing its code in. Leaves the page on the card. */
export async function addCard(
  page: Page,
  card: { name: string; code: string; kind?: string; holder?: string; private?: boolean },
): Promise<void> {
  await page.goto("/#/");
  await page.getByRole("link", { name: "+ Add a card" }).click();
  await page.getByRole("button", { name: "Type it in" }).click();
  await page.getByLabel("Name", { exact: true }).fill(card.name);
  if (card.holder) await page.getByLabel("Whose card").selectOption({ label: card.holder });
  if (card.private) await page.getByLabel("Private: only I see it").check();
  if (card.kind) await page.getByLabel("Kind").selectOption({ label: card.kind });
  await page.getByLabel("Code", { exact: true }).fill(card.code);
  await page.getByRole("button", { name: "Add the card" }).click();
  await page.locator("svg.barcode").waitFor();
}

/** A PNG of a code, drawn by zxing in the test process. */
export async function codeImage(format: string, text: string, scale = 4): Promise<Buffer> {
  const { prepareZXingModule, writeBarcode } = await import("zxing-wasm/full");
  const wasm = join(import.meta.dir, "../node_modules/zxing-wasm/dist/full/zxing_full.wasm");
  prepareZXingModule({ overrides: { wasmBinary: await Bun.file(wasm).arrayBuffer() } });
  const written = await writeBarcode(text, { format: format as never, scale, addQuietZones: true });
  if (!written.image) throw new Error(written.error);
  return Buffer.from(await written.image.arrayBuffer());
}

/** A video file for Chromium's fake camera: `image` held still in front of the lens, in the
 * Y4M format Chromium reads (`--use-file-for-fake-video-capture`). The frame is made in a
 * browser page, which can decode the PNG and draw it. */
export async function cameraFile(image: Buffer): Promise<string> {
  const page = await (await browser("chromium")).newPage();
  const width = 640;
  const height = 480;
  const yuv = await page.evaluate(
    async ({ png, width, height }) => {
      const bytes = Uint8Array.from(atob(png), (c) => c.charCodeAt(0));
      const bitmap = await createImageBitmap(new Blob([bytes], { type: "image/png" }));
      const canvas = new OffscreenCanvas(width, height);
      const g = canvas.getContext("2d");
      if (!g) throw new Error("no canvas");
      g.fillStyle = "#fff";
      g.fillRect(0, 0, width, height);
      const scale = Math.min((width * 0.8) / bitmap.width, (height * 0.8) / bitmap.height);
      const w = bitmap.width * scale;
      const h = bitmap.height * scale;
      g.imageSmoothingEnabled = false;
      g.drawImage(bitmap, (width - w) / 2, (height - h) / 2, w, h);
      const rgba = g.getImageData(0, 0, width, height).data;
      const out = new Uint8Array(width * height * 1.5);
      for (let i = 0; i < width * height; i++) {
        const r = rgba[i * 4] ?? 0;
        const gr = rgba[i * 4 + 1] ?? 0;
        const b = rgba[i * 4 + 2] ?? 0;
        out[i] = Math.round(0.299 * r + 0.587 * gr + 0.114 * b);
      }
      out.fill(128, width * height);
      let text = "";
      for (let i = 0; i < out.length; i += 0x8000) {
        text += String.fromCharCode(...out.subarray(i, i + 0x8000));
      }
      return btoa(text);
    },
    { png: image.toString("base64"), width, height },
  );
  await page.close();
  const frame = Buffer.from(yuv, "base64");
  const dir = await mkdtemp(join(tmpdir(), "loyalty-camera-"));
  cleanups.push(() => rm(dir, { recursive: true, force: true }));
  const path = join(dir, "camera.y4m");
  const header = Buffer.from(`YUV4MPEG2 W${width} H${height} F10:1 Ip A1:1 C420jpeg\n`);
  const frames = Array.from({ length: 5 }, () => [Buffer.from("FRAME\n"), frame]).flat();
  await Bun.write(path, Buffer.concat([header, ...frames]));
  return path;
}

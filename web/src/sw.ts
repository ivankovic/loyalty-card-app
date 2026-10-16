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

// The service worker: what makes the app open with no signal.
//
// - The app itself (PRECACHE: the page, its script and styles, the barcode WebAssembly, the
//   icons) is stored when the worker installs, and served from storage. A new release is a new
//   worker with a new cache, which replaces the old one.
// - Pages are asked of the server first, and come from storage when it does not answer.
// - Photos are stored the first time they load; their URLs are their content's hash, so a
//   stored photo is never stale.
// - The API is never stored here: the app keeps its own copy of the cards (store.ts), and a
//   stored answer about who is signed in would outlive signing out.

declare const PRECACHE: string[];
declare const CACHE: string;

interface ExtendableEvent extends Event {
  waitUntil(promise: Promise<unknown>): void;
}
interface FetchEvent extends ExtendableEvent {
  request: Request;
  respondWith(response: Promise<Response>): void;
}
interface Worker {
  addEventListener(type: "install" | "activate", listener: (event: ExtendableEvent) => void): void;
  addEventListener(type: "fetch", listener: (event: FetchEvent) => void): void;
  skipWaiting(): Promise<void>;
  clients: { claim(): Promise<void> };
}
const worker = self as unknown as Worker;

worker.addEventListener("install", (event) => {
  event.waitUntil(
    caches
      .open(CACHE)
      .then((cache) => cache.addAll(PRECACHE))
      .then(() => worker.skipWaiting()),
  );
});

worker.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(
          keys.filter((k) => k.startsWith("app-") && k !== CACHE).map((k) => caches.delete(k)),
        ),
      )
      .then(() => worker.clients.claim()),
  );
});

worker.addEventListener("fetch", (event) => {
  const request = event.request;
  const url = new URL(request.url);
  if (request.method !== "GET" || url.origin !== location.origin) return;
  if (url.pathname.startsWith("/api/")) return;
  if (request.mode === "navigate") {
    event.respondWith(
      fetch(request).catch(async () => (await caches.match("/index.html")) ?? Response.error()),
    );
    return;
  }
  if (url.pathname.startsWith("/photos/")) {
    event.respondWith(
      caches.open("photos").then(async (cache) => {
        const stored = await cache.match(request);
        if (stored) return stored;
        const fresh = await fetch(request);
        if (fresh.ok) await cache.put(request, fresh.clone());
        return fresh;
      }),
    );
    return;
  }
  event.respondWith(caches.match(request).then((stored) => stored ?? fetch(request)));
});

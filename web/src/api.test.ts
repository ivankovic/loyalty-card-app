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
import { Api, ApiError, inputOf, Offline } from "./api";
import { card, FakeServer } from "./fake";

test("every request carries the header the server's CSRF guard asks for", async () => {
  const server = new FakeServer();
  await new Api(server.fetch).cards();
  expect(server.requests[0]?.headers["X-Loyalty"]).toBe("1");
});

test("a JSON body is sent as JSON, a photo as itself", async () => {
  const server = new FakeServer();
  const api = new Api(server.fetch);
  await api.login("marko", "pw").catch(() => {});
  await api.putPhoto(1, "front", new Blob(["x"], { type: "image/jpeg" })).catch(() => {});
  expect(server.requests[0]?.headers["Content-Type"]).toBe("application/json");
  expect(server.requests[0]?.body).toBe('{"name":"marko","password":"pw"}');
  expect(server.requests[1]?.headers["Content-Type"]).toBe("image/jpeg");
  expect(server.requests[1]?.path).toBe("/api/cards/1/photos/front");
});

test("a refusal carries the server's words", async () => {
  const api = new Api(new FakeServer().fetch);
  const err = await api.login("marko", "wrong").catch((e: unknown) => e);
  expect(err).toBeInstanceOf(ApiError);
  expect((err as ApiError).status).toBe(401);
  expect((err as ApiError).message).toBe("wrong name or password");
  const missing = await api.card(999).catch((e: unknown) => e);
  expect((missing as ApiError).message).toBe("no such card");
});

test("an answer without JSON still makes an error with the status", async () => {
  const api = new Api(new FakeServer().fetch);
  const err = await api.deleteCard(5).catch((e: unknown) => e);
  expect((err as ApiError).message).toBe("The server answered 500.");
});

test("no network, or a proxy whose server is down, is offline", async () => {
  const server = new FakeServer();
  server.reachable = false;
  expect(await new Api(server.fetch).cards().catch((e: unknown) => e)).toBeInstanceOf(Offline);
  const proxied = new FakeServer();
  const api = new Api((path, init) =>
    proxied.fetch(path === "/api/me" ? "/api/broken" : path, init),
  );
  expect(await api.me().catch((e: unknown) => e)).toBeInstanceOf(Offline);
});

test("a 204 answer is no value", async () => {
  expect(await new Api(new FakeServer().fetch).logout()).toBeUndefined();
});

test("an edit starts from the card as it is", () => {
  const c = card(3, "Miles", { holder: { id: 2, display_name: "Ana" }, private: true });
  expect(inputOf(c)).toEqual({
    holder: 2,
    private: true,
    name: "Miles",
    program: "",
    code_value: "4006381333931",
    code_format: "ean13",
    display_number: "",
    color: "#1c7455",
    notes: "",
  });
});

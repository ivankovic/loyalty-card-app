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

import { describe, expect, test } from "bun:test";
import { Api } from "./api";
import { card, FakeServer, HOUSEHOLD, ME } from "./fake";
import { filtered, MemoryKV, Mirror, ordered } from "./store";

function mirror(server = new FakeServer()) {
  const kv = new MemoryKV();
  return { server, kv, mirror: new Mirror(new Api(server.fetch), kv, () => 1000) };
}

describe("the offline mirror", () => {
  test("keeps what the server sent", async () => {
    const { mirror: m } = mirror();
    const snapshot = await m.load();
    expect(snapshot).toEqual({
      me: ME,
      household: HOUSEHOLD,
      cards: [card(1, "Coop")],
      savedAt: 1000,
    });
    expect(m.offline).toBe(false);
    expect(await m.cached()).toEqual(snapshot);
  });

  test("shows the last cards when the server cannot be reached", async () => {
    const { server, mirror: m } = mirror();
    await m.load();
    server.reachable = false;
    const snapshot = await m.load();
    expect(m.offline).toBe(true);
    expect(snapshot?.cards.map((c) => c.name)).toEqual(["Coop"]);
  });

  test("is empty offline when it never saw the server", async () => {
    const { server, mirror: m } = mirror();
    server.reachable = false;
    expect(await m.load()).toBeNull();
    expect(m.offline).toBe(true);
  });

  test("forgets everything when the session has ended", async () => {
    const { server, kv, mirror: m } = mirror();
    await m.load();
    server.signedIn = false;
    expect(await m.load()).toBeNull();
    expect(await kv.get("snapshot")).toBeUndefined();
  });

  test("keeps a changed card and drops a deleted one", async () => {
    const { mirror: m } = mirror();
    await m.load();
    await m.put(card(1, "Coop Supercard"));
    await m.put(card(2, "Migros"));
    await m.remove(1);
    expect((await m.cached())?.cards.map((c) => c.name)).toEqual(["Migros"]);
  });

  test("changes nothing before the first load", async () => {
    const { mirror: m } = mirror();
    await m.put(card(2, "Migros"));
    await m.remove(1);
    expect(await m.cached()).toBeNull();
  });

  test("is cleared on signing out", async () => {
    const { mirror: m } = mirror();
    await m.load();
    await m.clear();
    expect(await m.cached()).toBeNull();
  });
});

describe("the list", () => {
  const cards = [
    card(1, "banana", { use_count: 1 }),
    card(2, "Apple"),
    card(3, "cherry", { use_count: 5, holder: { id: 2, display_name: "Ana" } }),
    card(4, "Miles", { holder: { id: 1, display_name: "Marko" }, program: "Lufthansa" }),
  ];

  test("puts the most used first, then sorts by name without case", () => {
    expect(ordered(cards).map((c) => c.name)).toEqual(["cherry", "banana", "Apple", "Miles"]);
  });

  test("filters by holder", () => {
    expect(filtered(cards, "", "family").map((c) => c.id)).toEqual([1, 2]);
    expect(filtered(cards, "", 2).map((c) => c.id)).toEqual([3]);
    expect(filtered(cards, "", "all")).toHaveLength(4);
  });

  test("searches names, programmes and holders, every word", () => {
    expect(filtered(cards, "lufthansa", "all").map((c) => c.id)).toEqual([4]);
    expect(filtered(cards, "ana cherry", "all").map((c) => c.id)).toEqual([3]);
    expect(filtered(cards, "ana apple", "all")).toEqual([]);
  });
});

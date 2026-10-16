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

// An in-memory stand-in for the server, for the unit tests of the code that talks to it: it
// answers the few requests those tests make, the way the real one does, and records them.
// The real server is tested against the app end to end in e2e/.

import type { Card, Fetch, Household, Me } from "./api";

export const ME: Me = {
  id: 1,
  name: "marko",
  display_name: "Marko",
  is_admin: true,
  household: { id: 1, name: "Home" },
};

export const HOUSEHOLD: Household = {
  id: 1,
  name: "Home",
  members: [
    { id: 2, name: "ana", display_name: "Ana" },
    { id: 1, name: "marko", display_name: "Marko" },
  ],
};

export function card(id: number, name: string, extra: Partial<Card> = {}): Card {
  return {
    id,
    holder: null,
    private: false,
    name,
    program: "",
    code: { format: "ean13", value: "4006381333931" },
    display_number: "",
    color: "#1c7455",
    notes: "",
    front: null,
    back: null,
    use_count: 0,
    last_used_at: null,
    created_at: 1,
    updated_at: 1,
    ...extra,
  };
}

export class FakeServer {
  signedIn = true;
  reachable = true;
  cards: Card[] = [card(1, "Coop")];
  readonly requests: {
    method: string;
    path: string;
    headers: Record<string, string>;
    body: unknown;
  }[] = [];

  readonly fetch: Fetch = async (path, init) => {
    const method = init.method ?? "GET";
    const headers = (init.headers ?? {}) as Record<string, string>;
    this.requests.push({ method, path, headers, body: init.body });
    if (!this.reachable) throw new TypeError("Failed to fetch");
    const json = (status: number, body: unknown) =>
      new Response(JSON.stringify(body), {
        status,
        headers: { "Content-Type": "application/json" },
      });
    if (!this.signedIn && path !== "/api/login") return json(401, { error: "sign in first" });
    switch (`${method} ${path}`) {
      case "GET /api/me":
        return json(200, ME);
      case "GET /api/household":
        return json(200, HOUSEHOLD);
      case "GET /api/cards":
        return json(200, this.cards);
      case "POST /api/logout":
        return new Response(null, { status: 204 });
      case "POST /api/login":
        return json(401, { error: "wrong name or password" });
      case "GET /api/cards/999":
        return json(404, { error: "no such card" });
      case "GET /api/broken":
        return new Response("<html>bad gateway</html>", { status: 502 });
      default:
        return new Response("not json", { status: 500 });
    }
  };
}

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

// The server's API, as typed functions. The shapes here are the ones pinned in
// tests/golden/api.json on the Rust side: a change there is a change here.

export type CodeFormat =
  | "qr"
  | "ean13"
  | "ean8"
  | "upca"
  | "upce"
  | "code128"
  | "code39"
  | "code93"
  | "itf"
  | "codabar"
  | "pdf417"
  | "aztec"
  | "datamatrix";

export interface Card {
  id: number;
  holder: { id: number; display_name: string } | null;
  private: boolean;
  name: string;
  program: string;
  code: { format: CodeFormat; value: string };
  display_number: string;
  color: string;
  notes: string;
  front: string | null;
  back: string | null;
  use_count: number;
  last_used_at: number | null;
  created_at: number;
  updated_at: number;
}

export interface CardInput {
  holder: number | null;
  private: boolean;
  name: string;
  program: string;
  code_value: string;
  code_format: CodeFormat;
  display_number: string;
  color: string;
  notes: string;
}

export interface Me {
  id: number;
  name: string;
  display_name: string;
  is_admin: boolean;
  household: { id: number; name: string };
}

export interface Member {
  id: number;
  name: string;
  display_name: string;
}

export interface Household {
  id: number;
  name: string;
  members: Member[];
}

export interface InviteInfo {
  household: string;
  name: string;
  reset: boolean;
  expires_at: number;
}

/** The server answered, and said no. `message` is meant for the person using the app. */
export class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

/** The server could not be reached: the phone is offline, or the server is down. */
export class Offline extends Error {
  constructor() {
    super("You are offline.");
  }
}

/** How a request leaves the app. Tests pass a fake server; the app uses `fetch`. */
export type Fetch = (input: string, init: RequestInit) => Promise<Response>;

export class Api {
  constructor(private readonly fetcher: Fetch = (input, init) => fetch(input, init)) {}

  private async call<T>(method: string, path: string, body?: unknown): Promise<T> {
    const headers: Record<string, string> = { "X-Loyalty": "1" };
    let payload: BodyInit | undefined;
    if (body instanceof Blob) {
      payload = body;
      headers["Content-Type"] = body.type || "application/octet-stream";
    } else if (body !== undefined) {
      payload = JSON.stringify(body);
      headers["Content-Type"] = "application/json";
    }
    let response: Response;
    try {
      response = await this.fetcher(path, {
        method,
        headers,
        body: payload,
        credentials: "same-origin",
      });
    } catch {
      throw new Offline();
    }
    if (response.status === 204) return undefined as T;
    let parsed: unknown;
    try {
      parsed = await response.json();
    } catch {
      parsed = undefined;
    }
    if (!response.ok) {
      const message =
        typeof parsed === "object" && parsed !== null && "error" in parsed
          ? String((parsed as { error: unknown }).error)
          : `The server answered ${response.status}.`;
      // A proxy in front of a stopped server answers 502 or 504: as good as offline.
      if (response.status === 502 || response.status === 503 || response.status === 504) {
        throw new Offline();
      }
      throw new ApiError(response.status, message);
    }
    return parsed as T;
  }

  me = () => this.call<Me>("GET", "/api/me");
  login = (name: string, password: string) =>
    this.call<Me>("POST", "/api/login", { name, password });
  logout = () => this.call<void>("POST", "/api/logout");
  changePassword = (current: string, next: string) =>
    this.call<void>("POST", "/api/me/password", { current, new: next });
  household = () => this.call<Household>("GET", "/api/household");
  invite = (name: string, admin: boolean) =>
    this.call<{ link: string; expires_at: number }>("POST", "/api/invites", { name, admin });
  inviteInfo = (token: string) =>
    this.call<InviteInfo>("GET", `/api/invite/${encodeURIComponent(token)}`);
  acceptInvite = (token: string, password: string, displayName: string) =>
    this.call<Me>("POST", `/api/invite/${encodeURIComponent(token)}`, {
      password,
      display_name: displayName || null,
    });
  cards = () => this.call<Card[]>("GET", "/api/cards");
  card = (id: number) => this.call<Card>("GET", `/api/cards/${id}`);
  createCard = (input: CardInput) => this.call<Card>("POST", "/api/cards", input);
  updateCard = (id: number, input: CardInput) => this.call<Card>("PUT", `/api/cards/${id}`, input);
  deleteCard = (id: number) => this.call<void>("DELETE", `/api/cards/${id}`);
  used = (id: number) => this.call<Card>("POST", `/api/cards/${id}/used`, {});
  putPhoto = (id: number, side: "front" | "back", photo: Blob) =>
    this.call<Card>("PUT", `/api/cards/${id}/photos/${side}`, photo);
  deletePhoto = (id: number, side: "front" | "back") =>
    this.call<Card>("DELETE", `/api/cards/${id}/photos/${side}`);
}

/** The input that saves `card` unchanged: where an edit starts. */
export function inputOf(card: Card): CardInput {
  return {
    holder: card.holder?.id ?? null,
    private: card.private,
    name: card.name,
    program: card.program,
    code_value: card.code.value,
    code_format: card.code.format,
    display_number: card.display_number,
    color: card.color,
    notes: card.notes,
  };
}

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

// The app: what is on screen, chosen by the address.
//
//   /i/<token>          an invite (or a password reset) link
//   #/                  the cards
//   #/card/<id>         a card, ready for the till
//   #/card/<id>/edit    changing it
//   #/new               a new card
//   #/settings          the person, the household, signing out
//
// Everything shown comes from the offline mirror (store.ts), refreshed from the server when it
// answers, so the cards open at a till with no signal.

import { Api, ApiError, type Card, Offline } from "./api";
import { h, render } from "./dom";
import { IdbKV, type KV, MemoryKV, Mirror, type Snapshot } from "./store";
import { inviteView, loginView } from "./views/auth";
import { editView } from "./views/edit";
import { listView } from "./views/list";
import { settingsView } from "./views/settings";
import { showView } from "./views/show";

export class App {
  readonly api = new Api();
  readonly mirror: Mirror;
  snapshot: Snapshot | null = null;
  /** Undoes what the current screen set up (the camera, a wake lock) when another replaces it. */
  private cleanup: (() => void) | null = null;

  constructor(readonly root: HTMLElement) {
    this.mirror = new Mirror(this.api, keeper());
  }

  async start(): Promise<void> {
    // Any path under /i/, so that a link cut short when it was copied says so rather than
    // asking to sign in.
    const invite = /^\/i\/([^/]*)\/?$/.exec(location.pathname);
    if (invite) {
      inviteView(this, invite[1] ?? "");
      return;
    }
    window.addEventListener("hashchange", () => this.route());
    window.addEventListener("online", () => void this.refresh());
    await this.refresh();
  }

  /** Loads everything again and redraws the screen. */
  async refresh(): Promise<void> {
    try {
      this.snapshot = await this.mirror.load();
    } catch (err) {
      this.fail(err);
      return;
    }
    if (this.snapshot === null && !location.hash.startsWith("#/login")) {
      this.go("#/login");
      return;
    }
    this.route();
  }

  go(hash: string): void {
    if (location.hash === hash) this.route();
    else location.hash = hash;
  }

  /** Sets what the current screen must undo when it is left. */
  onLeave(undo: () => void): void {
    this.cleanup = undo;
  }

  route(): void {
    this.cleanup?.();
    this.cleanup = null;
    window.scrollTo(0, 0);
    const hash = location.hash || "#/";
    const card = /^#\/card\/(\d+)(\/edit)?$/.exec(hash);
    const found = card ? this.card(Number(card[1])) : undefined;
    if (hash === "#/login") {
      loginView(this);
    } else if (!this.snapshot) {
      this.go("#/login");
    } else if (card && !found) {
      this.go("#/");
    } else if (card && found) {
      if (card[2]) editView(this, found);
      else showView(this, found);
    } else if (hash === "#/new") {
      editView(this, null);
    } else if (hash === "#/settings") {
      settingsView(this);
    } else {
      listView(this);
    }
  }

  card(id: number): Card | undefined {
    return this.snapshot?.cards.find((c) => c.id === id);
  }

  /** Keeps a card the server returned, in the mirror and on screen. */
  async keep(card: Card): Promise<void> {
    await this.mirror.put(card);
    if (this.snapshot) {
      this.snapshot = {
        ...this.snapshot,
        cards: [...this.snapshot.cards.filter((c) => c.id !== card.id), card],
      };
    }
  }

  async forget(id: number): Promise<void> {
    await this.mirror.remove(id);
    if (this.snapshot) {
      this.snapshot = { ...this.snapshot, cards: this.snapshot.cards.filter((c) => c.id !== id) };
    }
  }

  /** A screen for an error nobody can act on but by trying again. */
  fail(err: unknown): void {
    render(
      this.root,
      h(
        "main",
        { class: "page narrow" },
        h("h1", {}, "Something went wrong"),
        h("p", {}, message(err)),
        h("button", { class: "primary", onclick: () => location.reload() }, "Try again"),
      ),
    );
  }
}

/** Words for the person about an error. */
export function message(err: unknown): string {
  if (err instanceof Offline) return "You are offline. This needs a connection.";
  if (err instanceof ApiError) return capitalise(err.message);
  if (err instanceof Error) return err.message;
  return String(err);
}

function capitalise(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

/** IndexedDB, or memory in a browser that refuses it (some private modes). */
function keeper(): KV {
  return typeof indexedDB === "undefined" ? new MemoryKV() : new IdbKV();
}

/** The colours a card can have, as swatches in the editor. */
export const COLOURS = [
  "#1c7455",
  "#0a5bbd",
  "#5b3cc4",
  "#c2291e",
  "#e0700b",
  "#d9a400",
  "#2e2e2e",
  "#b0306f",
];

/** A short, friendly "when" for the offline banner. */
export function since(ms: number, now = Date.now()): string {
  const minutes = Math.round((now - ms) / 60000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours} h ago`;
  return new Date(ms).toLocaleDateString();
}

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

// The offline mirror: the last cards, household and person the server sent, kept on the phone,
// so that the cards still show at a till with no signal. Read-only while offline: changes need
// the server.

import { type Api, ApiError, type Card, type Household, type Me, Offline } from "./api";

/** Somewhere to keep values between visits. */
export interface KV {
  get(key: string): Promise<unknown>;
  set(key: string, value: unknown): Promise<void>;
  clear(): Promise<void>;
}

/** Values kept in memory: for tests, and for a browser that refuses IndexedDB (private mode). */
export class MemoryKV implements KV {
  private readonly values = new Map<string, unknown>();
  async get(key: string) {
    return structuredClone(this.values.get(key));
  }
  async set(key: string, value: unknown) {
    this.values.set(key, structuredClone(value));
  }
  async clear() {
    this.values.clear();
  }
}

/** Values kept in IndexedDB. */
export class IdbKV implements KV {
  private db: Promise<IDBDatabase> | null = null;

  private open(): Promise<IDBDatabase> {
    this.db ??= new Promise((resolve, reject) => {
      const request = indexedDB.open("loyalty", 1);
      request.onupgradeneeded = () => request.result.createObjectStore("kv");
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
    return this.db;
  }

  private async run<T>(
    mode: IDBTransactionMode,
    act: (s: IDBObjectStore) => IDBRequest,
  ): Promise<T> {
    const db = await this.open();
    return new Promise((resolve, reject) => {
      const request = act(db.transaction("kv", mode).objectStore("kv"));
      request.onsuccess = () => resolve(request.result as T);
      request.onerror = () => reject(request.error);
    });
  }

  get(key: string) {
    return this.run<unknown>("readonly", (s) => s.get(key));
  }
  async set(key: string, value: unknown) {
    await this.run("readwrite", (s) => s.put(value, key));
  }
  async clear() {
    await this.run("readwrite", (s) => s.clear());
  }
}

export interface Snapshot {
  me: Me;
  household: Household;
  cards: Card[];
  /** When the server last sent it, in milliseconds. */
  savedAt: number;
}

const KEY = "snapshot";

export class Mirror {
  /** Whether the last load came from the phone rather than the server. */
  offline = false;

  constructor(
    private readonly api: Api,
    private readonly kv: KV,
    private readonly clock: () => number = Date.now,
  ) {}

  /** Everything the app shows: from the server if it answers, else as last seen. `null` when
   * nobody is signed in. */
  async load(): Promise<Snapshot | null> {
    try {
      const [me, household, cards] = await Promise.all([
        this.api.me(),
        this.api.household(),
        this.api.cards(),
      ]);
      const snapshot = { me, household, cards, savedAt: this.clock() };
      await this.kv.set(KEY, snapshot);
      this.offline = false;
      return snapshot;
    } catch (err) {
      if (err instanceof Offline) {
        this.offline = true;
        return this.cached();
      }
      if (err instanceof ApiError && err.status === 401) {
        await this.kv.clear();
        this.offline = false;
        return null;
      }
      throw err;
    }
  }

  async cached(): Promise<Snapshot | null> {
    return ((await this.kv.get(KEY)) as Snapshot | undefined) ?? null;
  }

  /** Keeps a card the server just returned. */
  async put(card: Card): Promise<void> {
    const snapshot = await this.cached();
    if (!snapshot) return;
    const others = snapshot.cards.filter((c) => c.id !== card.id);
    await this.kv.set(KEY, { ...snapshot, cards: [...others, card] });
  }

  async remove(id: number): Promise<void> {
    const snapshot = await this.cached();
    if (!snapshot) return;
    await this.kv.set(KEY, { ...snapshot, cards: snapshot.cards.filter((c) => c.id !== id) });
  }

  /** Forgets everything: signing out leaves no cards on the phone. */
  async clear(): Promise<void> {
    await this.kv.clear();
  }
}

/** The cards in the order the list shows them: most used first, then by name. */
export function ordered(cards: Card[]): Card[] {
  return [...cards].sort(
    (a, b) =>
      b.use_count - a.use_count ||
      a.name.localeCompare(b.name, undefined, { sensitivity: "base" }) ||
      a.id - b.id,
  );
}

/** The cards a search and a holder filter leave. `holder` is a person's id, "family", or "all". */
export function filtered(cards: Card[], search: string, holder: number | "family" | "all"): Card[] {
  const words = search.toLowerCase().split(/\s+/).filter(Boolean);
  return cards.filter((card) => {
    if (holder === "family" && card.holder !== null) return false;
    if (typeof holder === "number" && card.holder?.id !== holder) return false;
    const text =
      `${card.name} ${card.program} ${card.holder?.display_name ?? ""} ${card.notes}`.toLowerCase();
    return words.every((w) => text.includes(w));
  });
}

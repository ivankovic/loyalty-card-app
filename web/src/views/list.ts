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

// The cards: a search, a filter by holder, and a tile per card. A tap opens it for the till.

import type { Card } from "../api";
import { type App, since } from "../app";
import { h, render } from "../dom";
import { filtered, ordered } from "../store";

let search = "";
let holder: number | "family" | "all" = "all";

export function listView(app: App): void {
  const snapshot = app.snapshot;
  if (!snapshot) return;
  const grid = h("ul", { class: "grid", "aria-label": "Cards" });
  const draw = () => {
    const shown = filtered(ordered(snapshot.cards), search, holder);
    render(
      grid,
      ...shown.map((card) => h("li", {}, tile(card))),
      shown.length === 0
        ? h(
            "li",
            { class: "empty" },
            snapshot.cards.length === 0 ? "No cards yet. Add the first one." : "No card matches.",
          )
        : null,
    );
  };
  const searchBox = h("input", {
    type: "search",
    placeholder: "Search",
    "aria-label": "Search the cards",
    value: search,
    oninput: () => {
      search = searchBox.value;
      draw();
    },
  });
  const chips = h("div", { class: "chips", role: "radiogroup", "aria-label": "Whose cards" });
  const choices: [string, number | "family" | "all"][] = [
    ["All", "all"],
    ["Family", "family"],
    ...snapshot.household.members.map((m): [string, number] => [m.display_name, m.id]),
  ];
  const drawChips = () =>
    render(
      chips,
      ...choices.map(([label, value]) =>
        h(
          "button",
          {
            class: "chip",
            role: "radio",
            "aria-checked": String(value === holder),
            onclick: () => {
              holder = value;
              drawChips();
              draw();
            },
          },
          label,
        ),
      ),
    );
  drawChips();
  draw();
  render(
    app.root,
    h(
      "main",
      { class: "page" },
      h(
        "header",
        { class: "bar" },
        h("h1", {}, snapshot.household.name),
        h("a", { class: "icon-button", href: "#/settings", "aria-label": "Settings" }, "⚙"),
      ),
      app.mirror.offline
        ? h(
            "p",
            { class: "banner", role: "status" },
            `Offline: these are the cards as of ${since(snapshot.savedAt)}. Changes need a connection.`,
          )
        : null,
      h("div", { class: "filters" }, searchBox, chips),
      grid,
      app.mirror.offline ? null : h("a", { class: "fab", href: "#/new" }, "+ Add a card"),
    ),
  );
}

function tile(card: Card): HTMLElement {
  const link = h(
    "a",
    { class: "tile", href: `#/card/${card.id}` },
    h("span", { class: "tile-name" }, card.name),
    card.program ? h("span", { class: "tile-program" }, card.program) : null,
    h(
      "span",
      { class: "tile-holder" },
      card.holder ? card.holder.display_name : "Family",
      card.private ? " · private" : "",
    ),
  );
  link.style.setProperty("--card", card.color);
  return link;
}

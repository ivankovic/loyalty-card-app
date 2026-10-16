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

// A card ready for the till: the code as large as the screen allows, black on white, the
// number under it, and the screen kept on. A web app cannot raise the screen's brightness, so
// the first time the app says how.

import type { Card } from "../api";
import type { App } from "../app";
import { barcodeElement, complete, info } from "../barcode";
import { h, render } from "../dom";

const HINT = "loyalty.brightness-hint";

export function showView(app: App, card: Card): void {
  const code = h(
    "div",
    { class: "code", "aria-live": "polite" },
    h("p", { class: "muted" }, "Drawing the code…"),
  );
  const linear = info(card.code.format).linear;
  const number = card.display_number || (linear ? complete(card.code.format, card.code.value) : "");
  const photo = h("div", { class: "photo", hidden: true });
  const showPhoto = (hash: string, side: string) => {
    render(
      photo,
      h("img", { src: `/photos/${hash}`, alt: `The card's ${side}` }),
      h("button", { onclick: () => photo.setAttribute("hidden", "") }, "Close"),
    );
    photo.removeAttribute("hidden");
  };
  const page = h(
    "main",
    { class: "show" },
    h(
      "header",
      { class: "bar" },
      h("a", { class: "back", href: "#/" }, "‹ Cards"),
      h("a", { class: "icon-button", href: `#/card/${card.id}/edit`, "aria-label": "Edit" }, "✎"),
    ),
    h("h1", {}, card.name),
    h(
      "p",
      { class: "muted" },
      [card.program, card.holder?.display_name ?? "Family"].filter(Boolean).join(" · "),
    ),
    code,
    number ? h("p", { class: "number" }, number) : null,
    linear
      ? h(
          "button",
          { class: "quiet", onclick: () => page.classList.toggle("turned") },
          "Turn it sideways",
        )
      : null,
    h(
      "div",
      { class: "row" },
      card.front
        ? h("button", { onclick: () => showPhoto(card.front ?? "", "front") }, "Front")
        : null,
      card.back ? h("button", { onclick: () => showPhoto(card.back ?? "", "back") }, "Back") : null,
    ),
    card.notes ? h("p", { class: "notes" }, card.notes) : null,
    hint(),
    photo,
  );
  render(app.root, page);
  barcodeElement(card.code.format, card.code.value).then(
    (svg) => render(code, svg),
    (err: unknown) =>
      render(code, h("p", { class: "error" }, `The code cannot be drawn: ${String(err)}`)),
  );
  const release = keepAwake();
  app.onLeave(release);
  if (!app.mirror.offline) {
    app.api.used(card.id).then(
      (updated) => app.keep(updated),
      () => {},
    );
  }
}

function hint(): HTMLElement | null {
  try {
    if (localStorage.getItem(HINT)) return null;
    localStorage.setItem(HINT, "seen");
  } catch {
    // Storage refused: show the hint every time rather than never.
  }
  return h(
    "p",
    { class: "hint", role: "note" },
    "☀ Turn the screen's brightness up: the till reads the code more easily.",
  );
}

/** Keeps the screen on while the card is shown. Returns what lets it sleep again. */
function keepAwake(): () => void {
  let lock: WakeLockSentinel | null = null;
  let left = false;
  const request = () => {
    navigator.wakeLock?.request("screen").then(
      (l) => {
        if (left) void l.release();
        else lock = l;
      },
      () => {},
    );
  };
  // A lock ends when the page is hidden; take it again when it comes back.
  const again = () => {
    if (document.visibilityState === "visible") request();
  };
  request();
  document.addEventListener("visibilitychange", again);
  return () => {
    left = true;
    document.removeEventListener("visibilitychange", again);
    void lock?.release();
  };
}

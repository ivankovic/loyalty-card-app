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

// Adding or changing a card: who holds it, the code (scanned, read from a photo, or typed),
// the front and back photos, a colour.

import { type Card, type CardInput, type CodeFormat, inputOf } from "../api";
import { type App, COLOURS, message } from "../app";
import { FORMATS, problem, scan } from "../barcode";
import { type Child, h, render } from "../dom";
import { shrink } from "../photo";
import { type Found, Scanner } from "../scanner";

type Side = "front" | "back";

export function editView(app: App, card: Card | null): void {
  const snapshot = app.snapshot;
  if (!snapshot) return;
  const { me, household } = snapshot;
  const start: CardInput = card
    ? inputOf(card)
    : {
        holder: null,
        private: false,
        name: "",
        program: "",
        code_value: "",
        code_format: "ean13",
        display_number: "",
        color: COLOURS[0] ?? "#1c7455",
        notes: "",
      };
  /** Photos chosen for a card not saved yet: sent once it exists. */
  const pending: Partial<Record<Side, Blob>> = {};
  let saved = card;

  const name = h("input", { name: "name", required: true, maxlength: 60, value: start.name });
  const program = h("input", {
    name: "program",
    maxlength: 60,
    value: start.program,
    placeholder: "Optional",
  });
  const holder = h(
    "select",
    { name: "holder" },
    h("option", { value: "" }, "Family"),
    ...household.members.map((m) => h("option", { value: String(m.id) }, m.display_name)),
  );
  holder.value = start.holder === null ? "" : String(start.holder);
  const isPrivate = h("input", { type: "checkbox", name: "private", checked: start.private });
  const privateRow = h("label", { class: "check" }, isPrivate, "Private: only I see it");
  const syncPrivate = () => {
    const mine = holder.value === String(me.id);
    privateRow.hidden = !mine;
    if (!mine) isPrivate.checked = false;
  };
  holder.addEventListener("change", syncPrivate);
  syncPrivate();

  const format = h(
    "select",
    { name: "code_format" },
    ...FORMATS.map((f) => h("option", { value: f.id }, f.label)),
  );
  format.value = start.code_format;
  const value = h("textarea", {
    name: "code_value",
    rows: 1,
    required: true,
    value: start.code_value,
    autocapitalize: "none",
    spellcheck: "false",
  });
  const codeProblem = h("p", { class: "error", role: "alert" });
  const check = () => {
    codeProblem.textContent =
      value.value.trim() === ""
        ? ""
        : (problem(format.value as CodeFormat, value.value.trim()) ?? "");
  };
  value.addEventListener("input", check);
  format.addEventListener("change", check);
  check();
  const useFound = (found: Found) => {
    format.value = found.format;
    value.value = found.value;
    check();
  };

  const number = h("input", {
    name: "display_number",
    maxlength: 64,
    value: start.display_number,
    placeholder: "If it differs from the code",
  });
  const notes = h("textarea", { name: "notes", rows: 2, maxlength: 2000, value: start.notes });

  let colour = start.color;
  const swatches = h("div", { class: "swatches", role: "radiogroup", "aria-label": "Colour" });
  const custom = h("input", { type: "color", value: colour, "aria-label": "Another colour" });
  const drawSwatches = () =>
    render(
      swatches,
      ...COLOURS.map((c) => {
        const swatch = h("button", {
          type: "button",
          class: "swatch",
          role: "radio",
          "aria-checked": String(c === colour),
          "aria-label": c,
          onclick: () => {
            colour = c;
            drawSwatches();
          },
        });
        swatch.style.setProperty("--card", c);
        return swatch;
      }),
      custom,
    );
  custom.addEventListener("input", () => {
    colour = custom.value;
    drawSwatches();
  });
  drawSwatches();

  const status = h("p", { class: "error", role: "alert" });
  const photos = h("div", { class: "photos" });
  const drawPhotos = () => render(photos, photoSlot("front", "Front"), photoSlot("back", "Back"));
  const photoSlot = (side: Side, label: string): HTMLElement => {
    const hash = saved?.[side] ?? null;
    const local = pending[side];
    const picker = h("input", {
      type: "file",
      accept: "image/*",
      capture: "environment",
      hidden: true,
    });
    picker.addEventListener("change", async () => {
      const file = picker.files?.[0];
      if (!file) return;
      status.textContent = "Preparing the photo…";
      try {
        const small = await shrink(file);
        if (value.value.trim() === "") {
          const [found] = await scan(small);
          if (found) useFound(found);
        }
        if (saved) {
          saved = await app.api.putPhoto(saved.id, side, small);
          await app.keep(saved);
        } else {
          pending[side] = small;
        }
        status.textContent = "";
      } catch (err) {
        status.textContent = message(err);
      }
      drawPhotos();
    });
    const preview: Child = local
      ? h("img", { src: URL.createObjectURL(local), alt: `${label} (not saved yet)` })
      : hash
        ? h("img", { src: `/photos/${hash}`, alt: label })
        : h("span", { class: "muted" }, "No photo");
    return h(
      "div",
      { class: "photo-slot" },
      h("span", { class: "label" }, label),
      preview,
      picker,
      h(
        "button",
        { type: "button", onclick: () => picker.click() },
        hash || local ? "Replace" : "Add a photo",
      ),
      hash || local
        ? h(
            "button",
            {
              type: "button",
              class: "quiet",
              onclick: async () => {
                if (local) delete pending[side];
                else if (saved) {
                  try {
                    saved = await app.api.deletePhoto(saved.id, side);
                    await app.keep(saved);
                  } catch (err) {
                    status.textContent = message(err);
                  }
                }
                drawPhotos();
              },
            },
            "Remove",
          )
        : null,
    );
  };
  drawPhotos();

  const save = h("button", { class: "primary", type: "submit" }, card ? "Save" : "Add the card");
  const input = (): CardInput => ({
    holder: holder.value === "" ? null : Number(holder.value),
    private: isPrivate.checked,
    name: name.value.trim(),
    program: program.value.trim(),
    code_value: value.value.trim(),
    code_format: format.value as CodeFormat,
    display_number: number.value.trim(),
    color: colour,
    notes: notes.value,
  });
  const form = h(
    "form",
    {
      class: "stack",
      onsubmit: async (event: Event) => {
        event.preventDefault();
        const wanted = input();
        const wrong = problem(wanted.code_format, wanted.code_value);
        if (wrong) {
          codeProblem.textContent = wrong;
          value.focus();
          return;
        }
        save.disabled = true;
        status.textContent = "";
        try {
          saved = saved
            ? await app.api.updateCard(saved.id, wanted)
            : await app.api.createCard(wanted);
          for (const side of ["front", "back"] as const) {
            const blob = pending[side];
            if (blob) {
              saved = await app.api.putPhoto(saved.id, side, blob);
              delete pending[side];
            }
          }
          await app.keep(saved);
          app.go(`#/card/${saved.id}`);
        } catch (err) {
          status.textContent = message(err);
          save.disabled = false;
        }
      },
    },
    h("label", {}, "Name", name),
    h("label", {}, "Programme", program),
    h("label", {}, "Whose card", holder),
    privateRow,
    h(
      "fieldset",
      {},
      h("legend", {}, "The code"),
      h(
        "button",
        { type: "button", class: "primary", onclick: () => openScanner(app, useFound) },
        "Scan it with the camera",
      ),
      h("label", {}, "Kind", format),
      h("label", {}, "Code", value),
      codeProblem,
      h("label", {}, "Number printed on the card", number),
    ),
    h("fieldset", {}, h("legend", {}, "Photos"), photos),
    h("fieldset", {}, h("legend", {}, "Colour"), swatches),
    h("label", {}, "Notes", notes),
    status,
    save,
  );

  render(
    app.root,
    h(
      "main",
      { class: "page narrow" },
      h(
        "header",
        { class: "bar" },
        h("a", { class: "back", href: card ? `#/card/${card.id}` : "#/" }, "‹ Back"),
      ),
      h("h1", {}, card ? `Change ${card.name}` : "A new card"),
      app.mirror.offline
        ? h("p", { class: "banner" }, "Offline: saving needs a connection.")
        : null,
      form,
      card
        ? h(
            "button",
            {
              class: "danger",
              onclick: async () => {
                if (!confirm(`Delete ${card.name}? This cannot be undone.`)) return;
                try {
                  await app.api.deleteCard(card.id);
                  await app.forget(card.id);
                  app.go("#/");
                } catch (err) {
                  status.textContent = message(err);
                }
              },
            },
            "Delete the card",
          )
        : null,
    ),
  );
  if (!card) openScanner(app, useFound);
}

/** The camera, full screen, until a code is found or the person gives up. */
function openScanner(app: App, done: (found: Found) => void): void {
  const video = h("video", {
    autoplay: true,
    playsinline: true,
    muted: true,
    "aria-label": "Camera",
  });
  const status = h(
    "p",
    { class: "scan-status", role: "status" },
    "Point the camera at the barcode.",
  );
  const torch = h("button", { type: "button", hidden: true }, "Light");
  const controller = new AbortController();
  const scanner = new Scanner(video);
  const close = () => {
    controller.abort();
    scanner.stop();
    overlay.remove();
  };
  const photo = h("input", {
    type: "file",
    accept: "image/*",
    capture: "environment",
    hidden: true,
  });
  photo.addEventListener("change", async () => {
    const file = photo.files?.[0];
    if (!file) return;
    status.textContent = "Reading the photo…";
    try {
      const [found] = await scan(await shrink(file));
      if (found) {
        close();
        done(found);
      } else {
        status.textContent = "No code found in that photo. Try closer, without glare.";
      }
    } catch (err) {
      status.textContent = message(err);
    }
  });
  const overlay = h(
    "div",
    { class: "scanner", role: "dialog", "aria-label": "Scan the code" },
    video,
    h("div", { class: "aim", "aria-hidden": "true" }),
    status,
    h(
      "div",
      { class: "row" },
      torch,
      photo,
      h("button", { type: "button", onclick: () => photo.click() }, "Use a photo"),
      h("button", { type: "button", onclick: close }, "Type it in"),
    ),
  );
  let lit = false;
  torch.addEventListener("click", async () => {
    lit = !lit;
    await scanner.torch(lit).catch(() => {});
  });
  document.body.append(overlay);
  app.onLeave(close);
  scanner
    .start()
    .then(async () => {
      torch.hidden = !scanner.hasTorch();
      const found = await scanner.next(controller.signal);
      close();
      done(found);
    })
    .catch((err: unknown) => {
      if (err instanceof DOMException && err.name === "AbortError") return;
      status.textContent = `${message(err)} You can use a photo or type the code in.`;
    });
}

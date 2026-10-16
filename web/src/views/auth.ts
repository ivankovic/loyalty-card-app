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

// Signing in, and arriving through an invite link.

import { type App, message } from "../app";
import { h, render } from "../dom";

export function loginView(app: App): void {
  const error = h("p", { class: "error", role: "alert" });
  const name = h("input", {
    name: "name",
    autocomplete: "username",
    autocapitalize: "none",
    required: true,
  });
  const password = h("input", {
    name: "password",
    type: "password",
    autocomplete: "current-password",
    required: true,
  });
  const submit = h("button", { class: "primary", type: "submit" }, "Sign in");
  const form = h(
    "form",
    {
      class: "stack",
      onsubmit: async (event: Event) => {
        event.preventDefault();
        submit.disabled = true;
        error.textContent = "";
        try {
          await app.api.login(name.value.trim(), password.value);
          await app.refresh();
          app.go("#/");
        } catch (err) {
          error.textContent = message(err);
        } finally {
          submit.disabled = false;
        }
      },
    },
    h("label", {}, "Name", name),
    h("label", {}, "Password", password),
    error,
    submit,
  );
  render(
    app.root,
    h(
      "main",
      { class: "page narrow" },
      h("h1", { class: "brand" }, "Cards"),
      h("p", { class: "muted" }, "Your household's loyalty cards."),
      form,
      h("p", { class: "muted small" }, "No account? Ask whoever runs this for an invite link."),
    ),
  );
  name.focus();
}

export async function inviteView(app: App, token: string): Promise<void> {
  render(
    app.root,
    h("main", { class: "page narrow" }, h("p", { class: "muted" }, "Opening the invite…")),
  );
  let info: Awaited<ReturnType<typeof app.api.inviteInfo>>;
  try {
    info = await app.api.inviteInfo(token);
  } catch (err) {
    render(
      app.root,
      h(
        "main",
        { class: "page narrow" },
        h("h1", {}, "This link does not work"),
        h("p", {}, message(err)),
        h("p", { class: "muted" }, "Ask for a new link."),
      ),
    );
    return;
  }
  const error = h("p", { class: "error", role: "alert" });
  const display = h("input", {
    name: "display_name",
    value: capital(info.name),
    autocomplete: "nickname",
  });
  const password = h("input", {
    name: "password",
    type: "password",
    autocomplete: "new-password",
    required: true,
    minlength: 8,
  });
  const repeat = h("input", {
    name: "repeat",
    type: "password",
    autocomplete: "new-password",
    required: true,
  });
  const submit = h(
    "button",
    { class: "primary", type: "submit" },
    info.reset ? "Set the password" : "Join",
  );
  const form = h(
    "form",
    {
      class: "stack",
      onsubmit: async (event: Event) => {
        event.preventDefault();
        error.textContent = "";
        if (password.value !== repeat.value) {
          error.textContent = "The two passwords differ.";
          return;
        }
        submit.disabled = true;
        try {
          await app.api.acceptInvite(token, password.value, info.reset ? "" : display.value.trim());
          location.replace("/#/");
        } catch (err) {
          error.textContent = message(err);
          submit.disabled = false;
        }
      },
    },
    info.reset ? null : h("label", {}, "Your name, as the others see it", display),
    h("label", {}, "Choose a password (8 characters or more)", password),
    h("label", {}, "The password again", repeat),
    error,
    submit,
  );
  render(
    app.root,
    h(
      "main",
      { class: "page narrow" },
      h("h1", { class: "brand" }, "Cards"),
      info.reset
        ? h("p", {}, "Choose a new password for ", h("b", {}, info.name), ".")
        : h("p", {}, "Join ", h("b", {}, info.household), " as ", h("b", {}, info.name), "."),
      form,
    ),
  );
  password.focus();
}

function capital(name: string): string {
  return name.charAt(0).toUpperCase() + name.slice(1);
}

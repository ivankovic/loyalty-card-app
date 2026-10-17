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

// The person and the household: the colours, the password, inviting someone (administrators),
// signing out.

import { type App, message } from "../app";
import { h, render } from "../dom";
import { chooseTheme, savedTheme, THEMES, type Theme } from "../theme";

export function settingsView(app: App): void {
  const snapshot = app.snapshot;
  if (!snapshot) return;
  const { me, household } = snapshot;

  const chosen = savedTheme();
  const theme = h(
    "select",
    { onchange: (event: Event) => chooseTheme((event.target as HTMLSelectElement).value as Theme) },
    ...THEMES.map(({ value, label }) => h("option", { value, selected: value === chosen }, label)),
  );
  const appearance = h(
    "div",
    { class: "stack" },
    h("h2", {}, "Appearance"),
    h("label", {}, "Colours", theme),
    h("p", { class: "muted small" }, "Automatic follows the phone's light or dark setting."),
  );

  const pwError = h("p", { class: "error", role: "alert" });
  const current = h("input", {
    type: "password",
    autocomplete: "current-password",
    required: true,
  });
  const next = h("input", {
    type: "password",
    autocomplete: "new-password",
    required: true,
    minlength: 8,
  });
  const password = h(
    "form",
    {
      class: "stack",
      onsubmit: async (event: Event) => {
        event.preventDefault();
        pwError.textContent = "";
        try {
          await app.api.changePassword(current.value, next.value);
          current.value = "";
          next.value = "";
          pwError.textContent = "The password is changed.";
        } catch (err) {
          pwError.textContent = message(err);
        }
      },
    },
    h("h2", {}, "Password"),
    h("label", {}, "Current password", current),
    h("label", {}, "New password", next),
    pwError,
    h("button", { type: "submit" }, "Change the password"),
  );

  const inviteName = h("input", {
    autocapitalize: "none",
    required: true,
    pattern: "[A-Za-z0-9._\\-]{1,32}",
  });
  const inviteAdmin = h("input", { type: "checkbox" });
  const invited = h("div", { class: "stack", "aria-live": "polite" });
  const invite = me.is_admin
    ? h(
        "form",
        {
          class: "stack",
          onsubmit: async (event: Event) => {
            event.preventDefault();
            try {
              const { link } = await app.api.invite(inviteName.value.trim(), inviteAdmin.checked);
              const share = () => navigator.share?.({ title: `Join ${household.name}`, url: link });
              render(
                invited,
                h("p", {}, "Send this link. It works once, for 7 days:"),
                h("input", {
                  class: "link",
                  readonly: true,
                  value: link,
                  onfocus: (e: Event) => (e.target as HTMLInputElement).select(),
                }),
                "share" in navigator
                  ? h("button", { type: "button", onclick: share }, "Share the link")
                  : null,
              );
              inviteName.value = "";
            } catch (err) {
              render(invited, h("p", { class: "error" }, message(err)));
            }
          },
        },
        h("h2", {}, "Invite someone"),
        h("label", {}, "The name they sign in with", inviteName),
        h("label", { class: "check" }, inviteAdmin, "They may invite others too"),
        h("button", { type: "submit" }, "Make an invite link"),
        invited,
      )
    : null;

  render(
    app.root,
    h(
      "main",
      { class: "page narrow" },
      h("header", { class: "bar" }, h("a", { class: "back", href: "#/" }, "‹ Cards")),
      h("h1", {}, me.display_name),
      h("p", { class: "muted" }, `Signed in as ${me.name}, in ${household.name}.`),
      appearance,
      app.mirror.offline ? h("p", { class: "banner" }, "Offline: these need a connection.") : null,
      password,
      invite,
      h(
        "button",
        {
          class: "danger",
          onclick: async () => {
            try {
              await app.api.logout();
            } catch {
              // Offline: forget the cards here anyway; the session ends on its own.
            }
            await app.mirror.clear();
            app.snapshot = null;
            app.go("#/login");
          },
        },
        "Sign out",
      ),
      h(
        "p",
        { class: "muted small" },
        `Loyalty Card App ${APP_VERSION}, free software under the AGPL-3.0: `,
        h("a", { href: "https://github.com/ivankovic/loyalty-card-app" }, "its source code"),
        ".",
      ),
    ),
  );
}

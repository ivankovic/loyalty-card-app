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

// The colour theme: Light Academia or Dark Academia, following the phone's light or dark
// setting unless this device chose one. The palettes themselves are in style.css; this sets
// `data-theme` on <html> for them, and the browser's bar colour to match.
//
// The choice belongs to the device, not the account (a phone and a laptop may want different
// ones), so it is kept in localStorage. Where that is unavailable the theme simply follows the
// phone.

export type Theme = "auto" | "light" | "dark";

export const THEMES: { value: Theme; label: string }[] = [
  { value: "auto", label: "Automatic" },
  { value: "light", label: "Light academia" },
  { value: "dark", label: "Dark academia" },
];

const KEY = "loyalty-theme";

/** The browser's bar colour for each palette: its background (style.css `--bg`). */
const BAR = { light: "#f3ece0", dark: "#1b1612" };

export function savedTheme(): Theme {
  try {
    const value = localStorage.getItem(KEY);
    return value === "light" || value === "dark" ? value : "auto";
  } catch {
    return "auto";
  }
}

export function chooseTheme(theme: Theme): void {
  try {
    if (theme === "auto") localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, theme);
  } catch {
    // Not remembered; it still applies until the app is closed.
  }
  applyTheme(theme);
}

export function applyTheme(theme: Theme = savedTheme()): void {
  const root = document.documentElement;
  if (theme === "auto") delete root.dataset.theme;
  else root.dataset.theme = theme;
  for (const meta of document.querySelectorAll<HTMLMetaElement>('meta[name="theme-color"]')) {
    const scheme = theme === "auto" ? (meta.dataset.scheme as "light" | "dark") : theme;
    meta.content = BAR[scheme];
  }
}

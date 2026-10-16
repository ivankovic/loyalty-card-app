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

// Barcodes: the symbologies the app knows, the checks a till's scanner would make, and the
// drawing of a code as a crisp SVG. Reading and writing are zxing-cpp compiled to WebAssembly
// (zxing-wasm), one library for every symbology, served from this app's own origin.

import type { CodeFormat } from "./api";
import { readBarcodes, writeBarcode } from "./zxing";

export interface FormatInfo {
  id: CodeFormat;
  label: string;
  /** zxing's name for it. */
  zxing: string;
  /** One-dimensional: drawn wide, shown best with the phone on its side. */
  linear: boolean;
}

export const FORMATS: FormatInfo[] = [
  { id: "ean13", label: "EAN-13", zxing: "EAN13", linear: true },
  { id: "qr", label: "QR code", zxing: "QRCode", linear: false },
  { id: "code128", label: "Code 128", zxing: "Code128", linear: true },
  { id: "ean8", label: "EAN-8", zxing: "EAN8", linear: true },
  { id: "upca", label: "UPC-A", zxing: "UPCA", linear: true },
  { id: "upce", label: "UPC-E", zxing: "UPCE", linear: true },
  { id: "code39", label: "Code 39", zxing: "Code39", linear: true },
  { id: "code93", label: "Code 93", zxing: "Code93", linear: true },
  { id: "itf", label: "ITF", zxing: "ITF", linear: true },
  { id: "codabar", label: "Codabar", zxing: "Codabar", linear: true },
  { id: "pdf417", label: "PDF417", zxing: "PDF417", linear: false },
  { id: "aztec", label: "Aztec", zxing: "Aztec", linear: false },
  { id: "datamatrix", label: "Data Matrix", zxing: "DataMatrix", linear: false },
];

export function info(format: CodeFormat): FormatInfo {
  const found = FORMATS.find((f) => f.id === format);
  if (!found) throw new Error(`unknown format ${format}`);
  return found;
}

/** The GS1 check digit (EAN, UPC) for the digits before it. */
export function checkDigit(body: string): number {
  let sum = 0;
  for (let i = 0; i < body.length; i++) {
    const digit = Number(body[body.length - 1 - i]);
    sum += i % 2 === 0 ? digit * 3 : digit;
  }
  return (10 - (sum % 10)) % 10;
}

const FULL_LENGTH: Partial<Record<CodeFormat, number>> = { ean13: 13, ean8: 8, upca: 12 };

/** Why `value` cannot be a `format` code, or null if it can. The same checks as the server's. */
export function problem(format: CodeFormat, value: string): string | null {
  const digits = /^[0-9]*$/.test(value);
  const need = (lengths: number[]) =>
    digits && lengths.includes(value.length)
      ? null
      : `${info(format).label} takes ${lengths.join(" or ")} digits`;
  if (value.length === 0) return "The code is empty";
  const full = FULL_LENGTH[format];
  if (full !== undefined) {
    const wrong = need([full - 1, full]);
    if (wrong) return wrong;
    if (value.length === full) {
      const expected = checkDigit(value.slice(0, -1));
      if (Number(value.at(-1)) !== expected) {
        return `the last digit should be ${expected}: the number has a typing mistake`;
      }
    }
    return null;
  }
  switch (format) {
    case "upce":
      return need([6, 7, 8]);
    case "itf":
      return digits && value.length >= 2 && value.length % 2 === 0
        ? null
        : "ITF takes an even number of digits";
    case "code39":
      return /^[0-9A-Z \-.$/+%]+$/.test(value)
        ? null
        : "Code 39 takes digits, capital letters and - . $ / + % and space";
    case "codabar": {
      const inner = value.replace(/^[A-D]/, "").replace(/[A-D]$/, "");
      return /^[0-9\-$:/.+]+$/.test(inner) ? null : "Codabar takes digits and - $ : / . +";
    }
    case "code128":
    case "code93":
      return /^[\x20-\x7e]+$/.test(value)
        ? null
        : `${info(format).label} takes letters, digits and punctuation without accents`;
    default:
      return null;
  }
}

/** The value with its check digit, for EAN and UPC codes saved without one. */
export function complete(format: CodeFormat, value: string): string {
  const full = FULL_LENGTH[format];
  if (full !== undefined && value.length === full - 1 && /^[0-9]+$/.test(value)) {
    return value + checkDigit(value);
  }
  return value;
}

/** A scanned code in the app's terms. zxing reads a UPC-A as an EAN-13 starting with 0 and
 * expands a UPC-E to thirteen digits; both are kept as EAN-13, which every till reads. */
export function fromZxing(
  zxingFormat: string,
  text: string,
): { format: CodeFormat; value: string } | null {
  if (zxingFormat === "UPCE" && /^[0-9]{13}$/.test(text)) return { format: "ean13", value: text };
  const found = FORMATS.find((f) => f.zxing === zxingFormat);
  if (!found || text.length === 0) return null;
  return { format: found.id, value: text };
}

/** The parts of zint's SVG that make the picture: its size and its bars or modules. */
export function parseZint(svg: string): { width: number; height: number; paths: string[] } {
  const size = /<svg[^>]*\swidth="([0-9.]+)"[^>]*\sheight="([0-9.]+)"/.exec(svg);
  if (!size) throw new Error("the barcode writer returned no SVG");
  const paths = [...svg.matchAll(/<path d="([^"]+)"/g)].map((m) => m[1] ?? "");
  return { width: Number(size[1]), height: Number(size[2]), paths };
}

const xmlEscape = (text: string) =>
  text.replace(
    /[&<>"]/g,
    (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c] ?? c,
  );

/** The code drawn as SVG markup that scales to any size without blurring: black on white,
 * with the quiet zones the scanners need, and no styles (the page's CSP allows none inline). */
export async function barcodeMarkup(format: CodeFormat, value: string): Promise<string> {
  const written = await writeBarcode(complete(format, value), {
    format: info(format).zxing as never,
    addQuietZones: true,
    addHRT: false,
  });
  if (written.error) throw new Error(written.error);
  const { width, height, paths } = parseZint(written.svg);
  const kind = info(format).linear ? "linear" : "matrix";
  const label = xmlEscape(`${info(format).label} ${value}`);
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}" ` +
    `shape-rendering="crispEdges" role="img" aria-label="${label}" class="barcode ${kind}">` +
    `<rect width="${width}" height="${height}" fill="#fff"/>` +
    paths.map((d) => `<path d="${xmlEscape(d)}" fill="#000"/>`).join("") +
    "</svg>"
  );
}

/** The same, as an element for the page. */
export async function barcodeElement(format: CodeFormat, value: string): Promise<Element> {
  const markup = await barcodeMarkup(format, value);
  const parsed = new DOMParser().parseFromString(markup, "image/svg+xml");
  return document.importNode(parsed.documentElement, true);
}

/** Every code zxing finds in an image or a video frame, in the app's terms. */
export async function scan(
  input: Blob | ImageData,
): Promise<{ format: CodeFormat; value: string }[]> {
  const results = await readBarcodes(input, {
    tryHarder: true,
    tryRotate: true,
    tryInvert: true,
    maxNumberOfSymbols: 4,
    formats: [],
  });
  return results
    .filter((r) => r.isValid)
    .map((r) => fromZxing(r.format, r.text))
    .filter((r): r is { format: CodeFormat; value: string } => r !== null);
}

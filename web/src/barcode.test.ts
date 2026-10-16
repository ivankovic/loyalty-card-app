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

import { beforeAll, describe, expect, test } from "bun:test";
import type { CodeFormat } from "./api";
import {
  barcodeMarkup,
  checkDigit,
  complete,
  FORMATS,
  fromZxing,
  parseZint,
  problem,
} from "./barcode";
import { assertGolden } from "./golden";
import { zxingForTests } from "./testing";
import { readBarcodes, writeBarcode } from "./zxing";

beforeAll(zxingForTests);

describe("the checks a till's scanner makes", () => {
  const table: [CodeFormat, string, string | null][] = [
    ["ean13", "4006381333931", null],
    ["ean13", "400638133393", null],
    ["ean13", "4006381333930", "the last digit should be 1: the number has a typing mistake"],
    ["ean13", "40063813339", "EAN-13 takes 12 or 13 digits"],
    ["ean8", "96385074", null],
    ["upca", "036000291452", null],
    ["upca", "036000291453", "the last digit should be 2: the number has a typing mistake"],
    ["upce", "0425261", null],
    ["upce", "04252", "UPC-E takes 6 or 7 or 8 digits"],
    ["itf", "1234", null],
    ["itf", "123", "ITF takes an even number of digits"],
    ["code39", "ABC-123", null],
    ["code39", "abc", "Code 39 takes digits, capital letters and - . $ / + % and space"],
    ["codabar", "A40156B", null],
    ["codabar", "AB", "Codabar takes digits and - $ : / . +"],
    ["code128", "Member #42", null],
    ["code128", "Café", "Code 128 takes letters, digits and punctuation without accents"],
    ["qr", "anything ✓", null],
    ["qr", "", "The code is empty"],
  ];
  for (const [format, value, expected] of table) {
    test(`${format} ${JSON.stringify(value)}`, () => {
      expect(problem(format, value)).toBe(expected);
    });
  }
});

test("the check digit is the GS1 one", () => {
  expect(checkDigit("400638133393")).toBe(1);
  expect(checkDigit("9638507")).toBe(4);
  expect(checkDigit("03600029145")).toBe(2);
});

test("a code saved without its check digit is completed for drawing", () => {
  expect(complete("ean13", "400638133393")).toBe("4006381333931");
  expect(complete("ean13", "4006381333931")).toBe("4006381333931");
  expect(complete("code128", "40063")).toBe("40063");
});

test("zxing's names are the app's, and UPC readings become EAN-13", () => {
  expect(fromZxing("QRCode", "hi")).toEqual({ format: "qr", value: "hi" });
  expect(fromZxing("EAN13", "0036000291452")).toEqual({ format: "ean13", value: "0036000291452" });
  expect(fromZxing("UPCE", "0042100005264")).toEqual({ format: "ean13", value: "0042100005264" });
  expect(fromZxing("UPCE", "04252614")).toEqual({ format: "upce", value: "04252614" });
  expect(fromZxing("MaxiCode", "x")).toBeNull();
  expect(fromZxing("QRCode", "")).toBeNull();
});

test("zint's SVG is taken apart into its size and its bars", () => {
  const svg =
    '<?xml version="1.0"?>\n<svg width="12" height="5" version="1.1" xmlns="http://www.w3.org/2000/svg">' +
    '<rect x="0" y="0" width="12" height="5" fill="#FFFFFF"/><path d="M1 0h1v5h-1Z"/><path d="M3 0h2v5h-2Z"/></svg>';
  expect(parseZint(svg)).toEqual({ width: 12, height: 5, paths: ["M1 0h1v5h-1Z", "M3 0h2v5h-2Z"] });
  expect(() => parseZint("<html/>")).toThrow("no SVG");
});

test("every symbology is drawn, and what is drawn reads back as what was saved", async () => {
  const samples: Record<CodeFormat, string> = {
    qr: "https://example.org/member/42",
    ean13: "4006381333931",
    ean8: "96385074",
    upca: "036000291452",
    upce: "04252614",
    code128: "MEMBER-42",
    code39: "ABC-123",
    code93: "CODE93",
    itf: "12345678",
    codabar: "A40156B",
    pdf417: "M1DOE/JOHN E1234567 ZRHLHRLX 0316 123Y012A0001 100",
    aztec: "LOYALTY 42",
    datamatrix: "LOYALTY 42",
  };
  for (const { id, zxing } of FORMATS) {
    const value = samples[id];
    const markup = await barcodeMarkup(id, value);
    expect(markup).toContain('shape-rendering="crispEdges"');
    expect(markup).not.toContain("style=");
    const image = (await writeBarcode(value, { format: zxing as never })).image;
    if (!image) throw new Error(`no image for ${id}`);
    const read = (await readBarcodes(image, { formats: [] })).map((r) =>
      fromZxing(r.format, r.text),
    );
    // zxing reads a UPC-A as the EAN-13 with a 0 in front, and a UPC-E expanded to EAN-13.
    const expected =
      id === "upca"
        ? { format: "ean13", value: `0${value}` }
        : id === "upce"
          ? { format: "ean13", value: "0042100005264" }
          : { format: id, value };
    expect(read).toContainEqual(expected as never);
  }
});

test("a drawn code is pinned", async () => {
  await assertGolden("ean13.svg", await barcodeMarkup("ean13", "400638133393"));
  await assertGolden("qr.svg", await barcodeMarkup("qr", "LOYALTY"));
});

test("a label is escaped in the markup", async () => {
  const markup = await barcodeMarkup("qr", 'a "quoted" <b>');
  expect(markup).toContain('aria-label="QR code a &quot;quoted&quot; &lt;b&gt;"');
});

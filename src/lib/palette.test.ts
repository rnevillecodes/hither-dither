// Checks the colour palette in styles.css against the app-theme contrast minimums.
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("../styles.css", import.meta.url), "utf8");
const root = css.match(/:root\s*\{([^}]*)\}/)?.[1] ?? "";
const palette = Object.fromEntries(
  [...root.matchAll(/(--[\w-]+):\s*(#[0-9a-fA-F]{6})\s*;/g)].map((m) => [m[1], m[2]]),
);

function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG contrast ratio between two palette variables. */
function contrast(fg: string, bg: string): number {
  const a = luminance(palette[fg]);
  const b = luminance(palette[bg]);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

const SURFACES = ["--bg", "--panel-bg", "--button-bg", "--button-hover"];

const RULES: [fg: string, bgs: string[], min: number][] = [
  ["--text", SURFACES, 7],
  ["--text-dim", SURFACES, 4.5],
  ["--accent-text", ["--accent"], 4.5],
  ["--accent", ["--bg", "--panel-bg", "--button-bg"], 3],
  ["--wf-playhead", ["--wf-lane-bg"], 3],
  ["--wf-clip-marker", ["--wf-lane-bg"], 3],
];

describe("palette contrast", () => {
  it("defines every colour the rules check as a hex value", () => {
    for (const [fg, bgs] of RULES) {
      for (const v of [fg, ...bgs]) expect(palette[v], v).toMatch(/^#[0-9a-fA-F]{6}$/);
    }
  });

  for (const [fg, bgs, min] of RULES) {
    for (const bg of bgs) {
      it(`${fg} on ${bg} is at least ${min}:1`, () => {
        expect(contrast(fg, bg)).toBeGreaterThanOrEqual(min);
      });
    }
  }
});

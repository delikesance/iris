import { describe, it, expect } from "vitest";
import { readdirSync, readFileSync, statSync } from "node:fs";
const SRC = `${process.cwd()}/src/`;

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((n: string) => {
    const p = `${dir}/${n}`;
    if (statSync(p).isDirectory()) return walk(p);
    return /\.(css|svelte|svg)$/.test(n) ? [p] : [];
  });
}

const files = walk(SRC.replace(/\/$/, "")).map(p => ({ path: p.slice(SRC.length), text: readFileSync(p, "utf8") }));
const cssSvelte = files.filter(f => !f.path.endsWith(".svg"));

function hexToRgb(h: string): [number, number, number] {
  const s = h.length === 3 ? [...h].map(c => c + c).join("") : h;
  return [0, 2, 4].map(i => parseInt(s.slice(i, i + 2), 16)) as [number, number, number];
}
const isRed = ([r, g, b]: number[]) => r > 150 && g < 90 && b < 90;
const isGreen = ([r, g, b]: number[]) => g > 150 && r < 90 && b < 90;

function violations(re: RegExp, bad: (m: RegExpExecArray) => boolean): string[] {
  return cssSvelte.flatMap(f => [...f.text.matchAll(re)].filter(m => bad(m as RegExpExecArray)).map(m => `${f.path}: ${m[0]}`));
}

describe("charte", () => {
  it("sources trouvées", () => expect(cssSvelte.length).toBeGreaterThan(0));

  it("border-radius toujours 0", () =>
    expect(violations(/border-radius\s*:\s*([^;}]+)/g, m => !/^0(px)?\s*(!important)?$/.test(m[1].trim()))).toEqual([]));

  it("box-shadow toujours none", () =>
    expect(violations(/box-shadow\s*:\s*([^;}]+)/g, m => !/^none\s*(!important)?$/.test(m[1].trim()))).toEqual([]));

  it("aucune couleur rouge ou verte (hex, rgb)", () => {
    const hex = violations(/#([0-9a-fA-F]{6}|[0-9a-fA-F]{3})\b/g, m => { const c = hexToRgb(m[1]); return isRed(c) || isGreen(c); });
    const rgb = violations(/rgba?\(\s*(\d+)[\s,]+(\d+)[\s,]+(\d+)/g, m => { const c = [+m[1], +m[2], +m[3]]; return isRed(c) || isGreen(c); });
    expect([...hex, ...rgb]).toEqual([]);
  });

  it("aucun nom de couleur rouge ou vert", () =>
    expect(violations(/:\s*(red|green|lime|crimson|tomato|darkred|darkgreen)\b/gi, () => true)).toEqual([]));

  it("le texte secondaire n'est pas plus sombre que #A3A3BD", () => {
    const lum = ([r, g, b]: number[]) => 0.2126 * r + 0.7152 * g + 0.0722 * b;
    const tokens = cssSvelte.find(f => f.path.endsWith("tokens.css"))!.text;
    for (const name of ["text-2", "text-3"]) {
      const m = new RegExp(`--${name}\\s*:\\s*#([0-9a-fA-F]{6})`).exec(tokens)!;
      expect(lum(hexToRgb(m[1]))).toBeGreaterThanOrEqual(lum(hexToRgb("A3A3BD")));
    }
  });

  it(".grain seulement sur body ou #page", () => {
    const bad = cssSvelte.flatMap(f => {
      const css = [...f.text.matchAll(/(^|[^\w-])\.grain\b/g)].length;
      const markup = [...f.text.matchAll(/class\s*=\s*"[^"]*\bgrain\b[^"]*"/g)].length;
      const css_ok = f.path.endsWith("grain.css");
      return (css && !css_ok) || markup ? [f.path] : [];
    });
    expect(bad).toEqual([]);
  });

  it("cibles tactiles >= 44px pour Button, Field, Toggle, Tabs", () => {
    const small = cssSvelte
      .filter(f => /ui[\\/](Button|Field|Toggle|Tabs)\.svelte$/.test(f.path))
      .flatMap(f => [...f.text.matchAll(/min-(?:height|width)\s*:\s*([^;}]+)/g)].filter(m => !m[1].includes("--tap")).map(m => `${f.path}: ${m[0]}`));
    expect(small).toEqual([]);
  });

  it("IrisMark.svelte et iris-mark.svg ont les mêmes tracés", () => {
    const paths = (t: string) => [...t.matchAll(/<path d="([^"]+)"/g)].map(m => m[1]);
    const svg = files.find(f => f.path.endsWith("iris-mark.svg"))!.text;
    const cmp = cssSvelte.find(f => f.path.endsWith("IrisMark.svelte"))!.text;
    expect(paths(cmp)).toEqual(paths(svg));
  });
});

/** Géométrie de placement : px écran (origine haut-gauche) <-> points PDF (origine bas-gauche). */
export type Rect = [number, number, number, number];
export const SIG_W = 150;
export const SIG_H = 50;

/** Rectangle de signature centré sur le clic, borné à la page. clic en px CSS relatifs à la page rendue. */
export function placeRect(clickX: number, clickY: number, scale: number, pageW: number, pageH: number, w = SIG_W, h = SIG_H): Rect {
  const cx = clickX / scale;
  const cy = pageH - clickY / scale;
  const x0 = Math.min(Math.max(cx - w / 2, 0), Math.max(pageW - w, 0));
  const y0 = Math.min(Math.max(cy - h / 2, 0), Math.max(pageH - h, 0));
  return [x0, y0, x0 + w, y0 + h];
}

export function rectToCss(r: Rect, scale: number, pageH: number) {
  return { left: r[0] * scale, top: (pageH - r[3]) * scale, width: (r[2] - r[0]) * scale, height: (r[3] - r[1]) * scale };
}

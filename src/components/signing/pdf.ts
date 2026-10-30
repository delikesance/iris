/** Enveloppe pdfjs : worker local (empaqueté par Vite, same-origin -> CSP worker-src 'self'). */
export type PageInfo = { width: number; height: number };
export type PdfHandle = {
  numPages: number;
  pageSize(n: number): Promise<PageInfo>;
  render(n: number, canvas: HTMLCanvasElement, scale: number): Promise<void>;
  destroy(): void;
};

export async function fetchPdfBytes(path: string): Promise<Uint8Array> {
  const { convertFileSrc } = await import("@tauri-apps/api/core");
  const res = await fetch(convertFileSrc(path));
  if (!res.ok) throw new Error(`Lecture du document impossible (${res.status})`);
  return new Uint8Array(await res.arrayBuffer());
}

export async function openPdf(bytes: Uint8Array): Promise<PdfHandle> {
  const pdfjs = await import("pdfjs-dist");
  pdfjs.GlobalWorkerOptions.workerSrc = new URL("pdfjs-dist/build/pdf.worker.min.mjs", import.meta.url).href;
  const task = pdfjs.getDocument({ data: bytes.slice() });
  const doc = await task.promise;
  return {
    numPages: doc.numPages,
    async pageSize(n) {
      const v = (await doc.getPage(n)).getViewport({ scale: 1 });
      return { width: v.width, height: v.height };
    },
    async render(n, canvas, scale) {
      const page = await doc.getPage(n);
      const viewport = page.getViewport({ scale });
      canvas.width = Math.floor(viewport.width);
      canvas.height = Math.floor(viewport.height);
      await page.render({ canvas, viewport }).promise;
    },
    destroy() { void task.destroy(); },
  };
}

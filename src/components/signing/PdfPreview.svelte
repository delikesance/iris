<!--
  PdfPreview : aperçu pdfjs d'un PDF. Props : bytes (Uint8Array | null) · interactive (bool : clic = poser la signature)
  · placed ({page (1-based), rect}|null : signature posée, encadrée) · pending (libellé du champ de l'autre partie, optionnel)
  · onplace (page 1-based, rect en points PDF) · label (aria-label du champ).
-->
<script lang="ts">
  import { onDestroy } from "svelte";
  import { openPdf, type PdfHandle } from "./pdf";
  import { placeRect, rectToCss, type Rect } from "./geometry";

  let { bytes, interactive = false, placed = null, pending = null, onplace, label = "Champ de signature" }: {
    bytes: Uint8Array | null; interactive?: boolean; placed?: { page: number; rect: Rect } | null;
    pending?: string | null; onplace?: (page: number, rect: Rect) => void; label?: string;
  } = $props();

  let canvas = $state<HTMLCanvasElement>();
  let wrap = $state<HTMLDivElement>();
  let handle: PdfHandle | null = null;
  let page = $state(1);
  let pages = $state(0);
  let size = $state({ width: 0, height: 0 });
  let scale = $state(1);
  let error = $state("");

  async function load(b: Uint8Array) {
    handle?.destroy();
    handle = null; error = "";
    try {
      handle = await openPdf(b);
      pages = handle.numPages;
      page = 1;
      await draw();
    } catch (e) { error = `Aperçu impossible : ${e instanceof Error ? e.message : String(e)}`; }
  }
  async function draw() {
    if (!handle || !canvas) return;
    size = await handle.pageSize(page);
    const avail = wrap?.clientWidth || 0;
    scale = avail > 0 ? Math.min(1.5, avail / size.width) : 1;
    await handle.render(page, canvas, scale);
  }
  $effect(() => { if (bytes && canvas) void load(bytes); });
  onDestroy(() => handle?.destroy());

  async function go(d: number) {
    const n = page + d;
    if (n < 1 || n > pages) return;
    page = n;
    await draw();
  }
  function click(e: MouseEvent) {
    if (!interactive || !onplace) return;
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    onplace(page, placeRect(e.clientX - r.left, e.clientY - r.top, scale, size.width, size.height));
  }
  const box = $derived(placed && placed.page === page ? rectToCss(placed.rect, scale, size.height) : null);
</script>

<div class="wrap" bind:this={wrap}>
  {#if error}<p class="err" role="alert">{error}</p>{/if}
  <div class="page" style:width="{size.width * scale}px" style:height="{size.height * scale}px">
    <canvas bind:this={canvas}></canvas>
    {#if interactive}
      <button type="button" class="hit" aria-label={label} onclick={click}></button>
    {/if}
    {#if box}
      <div class="field done" style:left="{box.left}px" style:top="{box.top}px" style:width="{box.width}px" style:height="{box.height}px">Signé</div>
    {/if}
    {#if pending && !box}
      <div class="field wait" data-testid="pending-field">{pending}</div>
    {/if}
  </div>
  {#if pages > 1}
    <div class="nav">
      <button type="button" onclick={() => go(-1)} disabled={page <= 1}>Précédente</button>
      <span>Page {page} sur {pages}</span>
      <button type="button" onclick={() => go(1)} disabled={page >= pages}>Suivante</button>
    </div>
  {/if}
</div>

<style>
  .wrap{display:flex;flex-direction:column;align-items:center;gap:8px;width:100%}
  .page{position:relative;background:var(--surface);max-width:100%}
  canvas{display:block;width:100%;height:100%}
  .hit{position:absolute;inset:0;width:100%;height:100%;background:transparent;border:1px dashed var(--accent-light);cursor:crosshair;padding:0}
  .hit:focus-visible{outline:2px solid var(--accent-light)}
  .field{position:absolute;display:flex;align-items:center;justify-content:center;font:400 12px var(--font-ui);pointer-events:none}
  .field.done{border:1px solid var(--accent);color:var(--accent)}
  .field.wait{right:8%;bottom:10%;width:34%;height:12%;border:1px dashed var(--text-3);color:var(--text-3);background:var(--bg)}
  .nav{display:flex;gap:12px;align-items:center;font:400 13px var(--font-ui);color:var(--text-2)}
  .nav button{min-height:var(--tap);padding:0 12px;background:none;border:1px solid var(--border);color:var(--text);cursor:pointer}
  .nav button:disabled{opacity:.5;cursor:default}
  .err{color:var(--text);margin:0}
</style>

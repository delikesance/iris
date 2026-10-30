<!--
  SignatureSection : signature manuscrite (image PNG transparente, tracée à la souris, au stylet ou au doigt).
  Enregistrée avec signature_image_save, relue avec signature_image_get.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { Button, SectionLabel } from "../../../components/ui";
  import { signatureImageGet, signatureImageSave } from "../../../lib/api";
  import { toast } from "../../../lib/stores";
  import { PREFIX, toBase64 } from "./signatureData";

  let canvas: HTMLCanvasElement;
  let drawing = false;
  let dirty = $state(false);
  let saved = $state<string | null>(null);

  const ctx = () => canvas.getContext("2d")!;
  const point = (e: PointerEvent) => {
    const r = canvas.getBoundingClientRect();
    return [((e.clientX - r.left) * canvas.width) / r.width, ((e.clientY - r.top) * canvas.height) / r.height];
  };

  function down(e: PointerEvent) {
    drawing = true;
    canvas.setPointerCapture(e.pointerId);
    const c = ctx();
    c.lineWidth = 3; c.lineCap = "round"; c.lineJoin = "round"; c.strokeStyle = "#0A0A8C";
    c.beginPath();
    c.moveTo(...(point(e) as [number, number]));
  }
  function move(e: PointerEvent) {
    if (!drawing) return;
    ctx().lineTo(...(point(e) as [number, number]));
    ctx().stroke();
    dirty = true;
  }
  const up = () => (drawing = false);

  function clear() {
    ctx().clearRect(0, 0, canvas.width, canvas.height);
    dirty = false;
  }

  async function save() {
    try {
      await signatureImageSave(toBase64(canvas.toDataURL("image/png")));
      saved = canvas.toDataURL("image/png");
      toast.set({ kind: "info", text: "Signature enregistrée." });
    } catch (e) {
      toast.set({ kind: "error", text: String(e) });
    }
  }

  onMount(async () => {
    try {
      const b64 = await signatureImageGet();
      if (b64) saved = PREFIX + b64;
    } catch (e) {
      toast.set({ kind: "error", text: String(e) });
    }
  });
</script>

<SectionLabel>Signature manuscrite</SectionLabel>
<p>Tracez votre signature ci-dessous. Elle est conservée sur cet ordinateur et sert à signer les documents.</p>
<canvas bind:this={canvas} width="600" height="200" aria-label="Zone de signature"
  onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={up}></canvas>
<div class="actions">
  <Button variant="secondary" onclick={clear} disabled={!dirty}>Effacer</Button>
  <Button onclick={save} disabled={!dirty}>Enregistrer la signature</Button>
</div>
{#if saved}
  <p class="cap">Signature enregistrée</p>
  <img src={saved} alt="Signature enregistrée" />
{/if}

<style>
  p{color:var(--text-2);max-width:60ch}
  canvas{display:block;width:100%;max-width:600px;aspect-ratio:3/1;background:var(--text);border:1px solid var(--border);touch-action:none;cursor:crosshair}
  .actions{display:flex;gap:12px;margin:16px 0}
  .cap{color:var(--text-3);font-size:13px;margin-bottom:6px}
  img{display:block;background:var(--text);max-width:300px;border:1px solid var(--border)}
</style>

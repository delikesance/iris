<!--
  SigningPanel : panneau de signature sous un message. Props : target (SigningTarget : pièce jointe) · filename
  · replyTo (adresse de l'expéditeur) · subject · inReplyTo (Message-ID, optionnel) · onclose (optionnel).
  Flux : import de la pièce (sign_prepare_attachment) -> aperçu pdfjs -> clic = pose de la signature manuscrite
  (Réglages › Signatures) -> « Répondre avec le document signé » ouvre la composition avec le PDF scellé joint.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { Button } from "../ui";
  import * as api from "../../lib/api";
  import { composeDraft, route, toast } from "../../lib/stores";
  import type { Document, SigningTarget } from "../../lib/types";
  import PdfPreview from "./PdfPreview.svelte";
  import { fetchPdfBytes } from "./pdf";
  import { openDocumentId } from "./state";
  import { needsSoffice, replyDraft, SOFFICE_MESSAGE } from "./format";
  import type { Rect } from "./geometry";

  let { target, filename, replyTo, subject, inReplyTo = null, onclose }: {
    target: SigningTarget; filename: string; replyTo: string; subject: string; inReplyTo?: string | null; onclose?: () => void;
  } = $props();

  let doc = $state<Document | null>(null);
  let bytes = $state<Uint8Array | null>(null);
  let placed = $state<{ page: number; rect: Rect } | null>(null);
  let blocked = $state("");
  let busy = $state(true);
  let hasImage = $state(true);
  let png: string | null = null;

  const signed = $derived(placed !== null);

  onMount(async () => {
    try {
      if (needsSoffice(filename) && !(await api.sofficeAvailable())) { blocked = SOFFICE_MESSAGE; return; }
      png = await api.signatureImageGet();
      hasImage = png !== null;
      doc = await api.signPrepareAttachment(target.folder, target.uid, target.index);
      bytes = await fetchPdfBytes(doc.path);
    } catch (e) { blocked = String(e); }
    finally { busy = false; }
  });

  async function place(page: number, rect: Rect) {
    if (!doc || !png || signed) return;
    busy = true;
    try {
      // page : 0-based côté back (docs/api-contract.md)
      doc = await api.signApply(doc.id, page - 1, rect, png);
      placed = { page, rect };
      bytes = await fetchPdfBytes(doc.path);
    } catch (e) { toast.set({ kind: "error", text: String(e) }); }
    finally { busy = false; }
  }
  async function reply() {
    if (!doc) return;
    try {
      const path = await api.documentExportForReply(doc.id);
      composeDraft.set(replyDraft(path, replyTo, subject, inReplyTo));
    } catch (e) { toast.set({ kind: "error", text: String(e) }); }
  }
  function detail() {
    if (!doc) return;
    openDocumentId.set(doc.id);
    route.set("signatures");
  }
</script>

<section class="panel" aria-label="Signature du document">
  <header>
    <strong>{filename}</strong>
    {#if onclose}<button type="button" class="x" onclick={onclose} aria-label="Fermer l'aperçu">Fermer</button>{/if}
  </header>

  {#if blocked}
    <p class="note" role="alert">{blocked}</p>
  {:else}
    {#if !hasImage}
      <p class="note">Aucune signature manuscrite enregistrée. Ajoutez-la dans Réglages › Signatures.
        <button type="button" class="link" onclick={() => route.set("settings")}>Ouvrir les réglages</button></p>
    {:else if !signed && doc}
      <p class="hint">Cliquez sur le document à l'endroit où signer.</p>
    {/if}
    <PdfPreview {bytes} interactive={!!doc && hasImage && !signed && !busy} {placed} onplace={place} />
    <div class="actions">
      <Button onclick={reply} disabled={!signed}>Répondre avec le document signé</Button>
      <Button variant="secondary" onclick={detail} disabled={!doc}>Voir le détail</Button>
    </div>
  {/if}
</section>

<style>
  .panel{background:var(--surface);border:1px solid var(--border);padding:16px;display:flex;flex-direction:column;gap:12px}
  header{display:flex;justify-content:space-between;align-items:center;font:500 14px var(--font-ui)}
  .x,.link{background:none;border:0;color:var(--accent-light);cursor:pointer;font:400 13px var(--font-ui);min-height:var(--tap)}
  .note,.hint{margin:0;color:var(--text-2);font:400 13px var(--font-ui)}
  .actions{display:flex;gap:12px;flex-wrap:wrap}
</style>

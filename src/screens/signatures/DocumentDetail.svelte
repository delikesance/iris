<!--
  DocumentDetail : page de signature (PDF p.20). Props : id (document) · onback.
  Aperçu + champs, signataires (ajout/retrait), options, niveaux (Avancée/Qualifiée désactivés : prestataire de
  services de confiance requis), bouton d'envoi actif après votre signature, journal de preuve.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { Avatar, Button, Field, SectionLabel, StateSquare, Toggle } from "../../components/ui";
  import PdfPreview from "../../components/signing/PdfPreview.svelte";
  import { fetchPdfBytes } from "../../components/signing/pdf";
  import type { Rect } from "../../components/signing/geometry";
  import { formatTime, meSigned, replyDraft } from "../../components/signing/format";
  import * as api from "../../lib/api";
  import { composeDraft, toast } from "../../lib/stores";
  import type { Document, ProofEvent } from "../../lib/types";

  let { id, onback }: { id: number; onback: () => void } = $props();

  let doc = $state<Document | null>(null);
  let events = $state<ProofEvent[]>([]);
  let bytes = $state<Uint8Array | null>(null);
  let placed = $state<{ page: number; rect: Rect } | null>(null);
  let me = $state("");
  let inOrder = $state(true);
  let remind = $state(true);
  let adding = $state(false);
  let newName = $state("");
  let newEmail = $state("");
  let png = $state<string | null>(null);

  const mine = $derived(doc ? meSigned(doc, me) : false);
  const others = $derived(doc ? doc.signers.filter(s => s.email.toLowerCase() !== me.toLowerCase()) : []);
  const pendingOther = $derived(others.find(s => !s.signed));
  const status = $derived(!doc ? "" : mine ? (others.every(s => s.signed) ? "Signé par tous" : `Signé par vous, en attente de ${pendingOther?.name ?? "l'autre partie"}`) : "À signer");

  async function refresh() {
    doc = await api.documentGet(id);
    events = (await api.proofGet(id)).events;
  }
  onMount(async () => {
    try {
      me = await api.account();
      await refresh();
      png = await api.signatureImageGet();
      if (doc) bytes = await fetchPdfBytes(doc.path);
    } catch (e) { toast.set({ kind: "error", text: String(e) }); }
  });

  async function place(page: number, rect: Rect) {
    if (!doc || !png || mine) return;
    try {
      doc = await api.signApply(doc.id, page - 1, rect, png);
      placed = { page, rect };
      bytes = await fetchPdfBytes(doc.path);
      events = (await api.proofGet(id)).events;
    } catch (e) { toast.set({ kind: "error", text: String(e) }); }
  }
  async function add() {
    if (!doc || !newEmail.trim()) return;
    try {
      doc = await api.signerAdd(doc.id, newName.trim() || newEmail.trim(), newEmail.trim());
      newName = ""; newEmail = ""; adding = false;
    } catch (e) { toast.set({ kind: "error", text: String(e) }); }
  }
  async function remove(email: string) {
    if (!doc) return;
    try { doc = await api.signerRemove(doc.id, email); }
    catch (e) { toast.set({ kind: "error", text: String(e) }); }
  }
  async function send() {
    if (!doc || !mine) return;
    try {
      const path = await api.documentExportForReply(doc.id);
      composeDraft.set(replyDraft(path, others.map(s => s.email).join(", "), doc.name.replace(/\.[^.]+$/, "")));
    } catch (e) { toast.set({ kind: "error", text: String(e) }); }
  }
</script>

<div class="screen">
  <button type="button" class="back" onclick={onback}>‹ Signatures</button>
  {#if doc}
    <header>
      <h1>{doc.name.replace(/\.[^.]+$/, "")}</h1>
      <span class="status">{status}</span>
    </header>
    <div class="cols">
      <div class="doc">
        <PdfPreview {bytes} interactive={!mine && png !== null} {placed} pending={pendingOther ? `En attente de ${pendingOther.name}` : null} onplace={place} />
        {#if png === null}<p class="note">Aucune signature manuscrite enregistrée : ajoutez-la dans Réglages › Signatures.</p>{/if}
      </div>
      <aside>
        <section>
          <SectionLabel>Signataires</SectionLabel>
          <ul class="signers">
            {#each doc.signers as s (s.email)}
              <li>
                <Avatar name={s.name} variant={s.email.toLowerCase() === me.toLowerCase() ? "me" : "gray"} />
                <div class="who"><strong>{s.email.toLowerCase() === me.toLowerCase() ? "Vous" : s.name}</strong><span>{s.email}</span></div>
                <StateSquare state={s.signed ? "done" : "wait"} label={s.signed ? "Signé" : "En attente"} />
                {#if s.email.toLowerCase() !== me.toLowerCase() && !s.signed}
                  <button type="button" class="rm" onclick={() => remove(s.email)} aria-label={`Retirer ${s.name}`}>Retirer</button>
                {/if}
              </li>
            {/each}
          </ul>
          {#if adding}
            <form onsubmit={(e) => { e.preventDefault(); void add(); }}>
              <Field label="Nom" bind:value={newName} />
              <Field label="Adresse e-mail" type="email" bind:value={newEmail} />
              <Button type="submit" disabled={!newEmail.trim()}>Ajouter</Button>
            </form>
          {:else}
            <Button variant="secondary" onclick={() => (adding = true)}>Ajouter un signataire</Button>
          {/if}
        </section>
        <section>
          <SectionLabel>Options</SectionLabel>
          <div class="opt"><span>Signer dans l'ordre</span><Toggle label="Signer dans l'ordre" bind:checked={inOrder} /></div>
          <div class="opt"><span>Relancer tous les 3 jours</span><Toggle label="Relancer tous les 3 jours" bind:checked={remind} /></div>
          <div class="levels" role="group" aria-label="Niveau de signature">
            <span class="lbl">Niveau de signature</span>
            <button type="button" class="lv on" aria-pressed="true">Simple</button>
            <button type="button" class="lv" disabled aria-pressed="false">Avancée</button>
            <button type="button" class="lv" disabled aria-pressed="false">Qualifiée</button>
          </div>
          <p class="note">Les niveaux Avancée et Qualifiée supposent un prestataire de services de confiance ; seule la signature simple est réalisable sans.</p>
        </section>
        <Button onclick={send} disabled={!mine}>{others[0] ? `Renvoyer à ${others[0].name} avec le document signé` : "Envoyer le document signé"}</Button>
        <section>
          <SectionLabel>Journal de preuve</SectionLabel>
          <ul class="proof">
            {#each events as ev (ev.id)}
              <li><time>{formatTime(ev.at)}</time> {ev.detail}</li>
            {:else}
              <li>Aucun événement.</li>
            {/each}
          </ul>
        </section>
      </aside>
    </div>
  {:else}
    <p class="note">Chargement…</p>
  {/if}
</div>

<style>
  .screen{padding:24px 40px;display:flex;flex-direction:column;gap:16px}
  .back{align-self:flex-start;background:none;border:0;color:var(--text-3);cursor:pointer;font:400 13px var(--font-ui);min-height:var(--tap);padding:0}
  header{display:flex;justify-content:space-between;align-items:center;gap:16px}
  h1{margin:0;font:300 36px var(--font-title);color:var(--text)}
  .status{background:var(--fill);border:1px solid var(--border);padding:6px 12px;font:400 13px var(--font-ui);color:var(--text-2)}
  .cols{display:grid;grid-template-columns:1fr 340px;gap:24px;align-items:start}
  .doc{background:var(--surface);border:1px solid var(--border);padding:16px}
  aside{display:flex;flex-direction:column;gap:12px}
  aside section{background:var(--surface);border:1px solid var(--border);padding:16px;display:flex;flex-direction:column;gap:12px}
  ul{list-style:none;margin:0;padding:0}
  .signers li{display:grid;grid-template-columns:32px 1fr auto auto;gap:12px;align-items:center;padding:6px 0}
  .who{display:flex;flex-direction:column;min-width:0}
  .who span{font:400 12px var(--font-ui);color:var(--text-3)}
  .rm{background:none;border:0;color:var(--accent-light);cursor:pointer;font:400 13px var(--font-ui);min-height:var(--tap)}
  form{display:flex;flex-direction:column;gap:8px}
  .opt{display:flex;justify-content:space-between;align-items:center;font:400 14px var(--font-ui)}
  .levels{display:flex;flex-wrap:wrap;gap:8px;align-items:center}
  .lbl{flex-basis:100%;font:400 13px var(--font-ui);color:var(--text-3)}
  .lv{min-height:var(--tap);padding:0 14px;background:transparent;border:1px solid var(--border);color:var(--text);font:500 13px var(--font-ui);cursor:pointer}
  .lv.on{background:var(--accent);border-color:var(--accent);color:var(--on-accent)}
  .lv:disabled{opacity:.5;cursor:not-allowed}
  .note{margin:0;font:400 12px var(--font-ui);color:var(--text-2)}
  .proof li{font:400 13px var(--font-ui);color:var(--text-2);padding:2px 0}
  .proof time{color:var(--text-3)}
</style>

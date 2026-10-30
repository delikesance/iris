<!--
  SignaturesScreen : suivi des documents (PDF p.19) et page de signature (PDF p.20, DocumentDetail).
  Onglets À signer / En attente / Signés, StateSquare, « 1 sur 2 », actions Signer / Relancer / Télécharger.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { Button, ScreenTitle, StateSquare, Tabs } from "../../components/ui";
  import * as api from "../../lib/api";
  import { toast } from "../../lib/stores";
  import type { Document, DocumentTab } from "../../lib/types";
  import { openDocumentId } from "../../components/signing/state";
  import { actionOf, formatTime, progressLabel, stateOf } from "../../components/signing/format";
  import DocumentDetail from "./DocumentDetail.svelte";

  const tabIds: DocumentTab[] = ["todo", "waiting", "signed"];
  const names: Record<DocumentTab, string> = { todo: "À signer", waiting: "En attente", signed: "Signés" };

  let lists = $state<Record<DocumentTab, Document[]>>({ todo: [], waiting: [], signed: [] });
  let active = $state<string>("todo");
  let openId = $state<number | null>(null);
  let loading = $state(true);

  const tabs = $derived(tabIds.map(id => ({ id, label: `${names[id]} ${lists[id].length}` })));
  const rows = $derived(lists[active as DocumentTab] ?? []);

  async function load() {
    try {
      const [todo, waiting, signed] = await Promise.all(tabIds.map(t => api.documentsList(t)));
      lists = { todo, waiting, signed };
    } catch (e) { toast.set({ kind: "error", text: String(e) }); }
    finally { loading = false; }
  }
  onMount(() => {
    const unsub = openDocumentId.subscribe(id => { if (id !== null) { openId = id; openDocumentId.set(null); } });
    void load();
    return unsub;
  });

  async function importDoc() {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const path = await open({ multiple: false, filters: [{ name: "Documents", extensions: ["pdf", "docx", "doc", "odt"] }] });
      if (typeof path !== "string") return;
      const d = await api.documentImport(path);
      await load();
      openId = d.id;
    } catch (e) { toast.set({ kind: "error", text: String(e) }); }
  }
  async function act(d: Document) {
    const tab = d.status;
    try {
      if (tab === "todo") { openId = d.id; return; }
      if (tab === "waiting") {
        await api.documentRemind(d.id);
        toast.set({ kind: "info", text: "Relance envoyée." });
        return;
      }
      const path = await api.documentExportForReply(d.id);
      const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
      await revealItemInDir(path);
    } catch (e) { toast.set({ kind: "error", text: String(e) }); }
  }
  function back() { openId = null; void load(); }
</script>

{#if openId !== null}
  <DocumentDetail id={openId} onback={back} />
{:else}
  <div class="screen">
    <header>
      <div>
        <ScreenTitle>Signatures</ScreenTitle>
        <p class="sub">Signez et faites signer vos documents sans quitter Iris.</p>
      </div>
      <Button onclick={importDoc}>+ Nouveau document</Button>
    </header>
    <p class="info">Les documents que vous signez depuis vos mails apparaissent ici. Formats signables : PDF, Word, OpenDocument. Les images, archives et tableurs ne se signent pas.</p>
    <Tabs items={tabs} bind:active />
    <ul>
      {#each rows as d (d.id)}
        <li>
          <div class="name"><strong>{d.name}</strong><span>Reçu le {formatTime(d.created)}</span></div>
          <StateSquare state={stateOf(d.status)} />
          <span class="count">{progressLabel(d)}</span>
          <Button variant={d.status === "todo" ? "primary" : "secondary"} onclick={() => act(d)}>{actionOf(d.status)}</Button>
        </li>
      {:else}
        <li class="empty">{loading ? "Chargement…" : "Aucun document dans cet onglet."}</li>
      {/each}
    </ul>
  </div>
{/if}

<style>
  .screen{padding:32px 40px;display:flex;flex-direction:column;gap:16px}
  header{display:flex;justify-content:space-between;align-items:flex-end}
  .sub{margin:4px 0 0;color:var(--text-2);font:400 14px var(--font-ui)}
  .info{margin:0;padding:12px 16px;background:var(--surface);border:1px solid var(--border);color:var(--text-2);font:400 13px var(--font-ui)}
  ul{list-style:none;margin:0;padding:0;background:var(--surface);border:1px solid var(--border)}
  li{display:grid;grid-template-columns:1fr 120px 80px 140px;align-items:center;gap:16px;padding:12px 16px;border-bottom:1px solid var(--border)}
  li:last-child{border-bottom:0}
  li.empty{display:block;color:var(--text-3)}
  .name{display:flex;flex-direction:column}
  .name span{font:400 12px var(--font-ui);color:var(--text-3)}
  .count{font:400 13px var(--font-ui);color:var(--text-2)}
</style>

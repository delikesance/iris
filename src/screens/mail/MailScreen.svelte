<!--
  MailScreen : trois colonnes (dossiers · liste · lecture), une seule sous 700 px.
  Données : list_folders / list_messages / get_message ; rafraîchi par l'événement `mail://changed`.
  Stores : currentFolder, selectedUid, selection, composeDraft, toast.
-->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { get } from "svelte/store";
  import { listen } from "@tauri-apps/api/event";
  import { bulkDelete, bulkMove, deleteMessage, getMessage, listFolders, listMessages, markSeen, syncFolder } from "../../lib/api";
  import { composeDraft, currentFolder, mailQuery, selectedUid, selection, toast } from "../../lib/stores";
  import type { Folder, Full, MessageRow } from "../../lib/types";
  import { Button, ScreenTitle } from "../../components/ui";
  import FolderList from "./FolderList.svelte";
  import MailList from "./MailList.svelte";
  import MailReader from "./MailReader.svelte";
  import { applyFilters, type FilterMode } from "./helpers";

  // Recherche posée par un autre écran (ex. « Voir tous les mails » d'un contact) : consommée une fois.
  function takeMailQuery(): string {
    const q = get(mailQuery);
    mailQuery.set("");
    return q;
  }

  let folders = $state<Folder[]>([]);
  let rows = $state<MessageRow[]>([]);
  let message = $state<Full | null>(null);
  let query = $state(takeMailQuery());
  let mode = $state<FilterMode>("all");

  const visible = $derived(applyFilters(rows, { mode, query }));
  const folder = $derived(folders.find((f) => f.name === $currentFolder));
  const archive = $derived(folders.find((f) => f.kind === "system" && f.label === "Archives"));
  const fail = (what: string) => (e: unknown) => toast.set({ kind: "error", text: `${what} : ${e}` });

  let rowsSeq = 0;
  async function loadRows() {
    const seq = ++rowsSeq;
    try {
      const result = await listMessages($currentFolder, query.trim() || undefined);
      if (seq === rowsSeq) rows = result;
    } catch (e) { fail("Chargement de la liste impossible")(e); }
  }

  const loadFolders = () => listFolders().then((f) => (folders = f)).catch(fail("Chargement des dossiers impossible"));

  const refresh = () => Promise.all([loadFolders(), loadRows()]);

  // Changement de dossier : sélection remise à zéro, synchronisation en arrière-plan.
  $effect(() => {
    const name = $currentFolder;
    selectedUid.set(null);
    selection.set(new Set());
    syncFolder(name).then(refresh).catch(fail("Synchronisation impossible"));
  });

  // Recherche : rechargement après une courte pause.
  $effect(() => {
    void query;
    const t = setTimeout(loadRows, 200);
    return () => clearTimeout(t);
  });

  // Lecture du message sélectionné ; il passe en « lu ».
  $effect(() => {
    const uid = $selectedUid;
    const name = $currentFolder;
    if (uid === null) { message = null; return; }
    getMessage(name, uid).then((m) => {
      if ($selectedUid === uid) message = m;
    }).catch(fail("Lecture impossible"));
    const row = untrack(() => rows.find((r) => r.uid === uid));
    if (row && !row.seen) {
      row.seen = true;
      markSeen(name, uid, true).then(loadFolders).catch(fail("Marquage impossible"));
    }
  });

  onMount(() => {
    let unlisten: (() => void) | undefined;
    listen<string>("mail://changed", refresh).then((u) => (unlisten = u)).catch(() => {});
    return () => unlisten?.();
  });

  function selectAll() {
    const all = visible.length > 0 && visible.every((r) => $selection.has(r.uid));
    selection.set(all ? new Set() : new Set(visible.map((r) => r.uid)));
  }

  function toggle(uid: number) {
    selection.update((s) => {
      const next = new Set(s);
      if (!next.delete(uid)) next.add(uid);
      return next;
    });
  }

  async function groupAction(run: (uids: number[]) => Promise<void>, what: string) {
    try {
      await run([...$selection]);
      selection.set(new Set());
      await refresh();
    } catch (e) { fail(what)(e); }
  }

  async function removeCurrent() {
    if ($selectedUid === null) return;
    try {
      await deleteMessage($currentFolder, $selectedUid);
      selectedUid.set(null);
      await refresh();
    } catch (e) { fail("Suppression impossible")(e); }
  }

  const compose = () => composeDraft.set({ to: "", subject: "", body: "", attachments: [] });
</script>

<div class="screen" class:reading={$selectedUid !== null}>
  <aside>
    <div class="title"><ScreenTitle size={36}>Mails</ScreenTitle></div>
    <FolderList {folders} current={$currentFolder} onselect={(n) => currentFolder.set(n)} />
  </aside>

  <div class="list">
    {#if $selection.size > 0}
      <div class="group" role="toolbar" aria-label="Actions groupées">
        <span>{$selection.size} sélectionné{$selection.size > 1 ? "s" : ""}</span>
        {#if archive && archive.name !== $currentFolder}
          <Button variant="secondary" onclick={() => groupAction((u) => bulkMove($currentFolder, u, archive.name), "Archivage impossible")}>Archiver</Button>
        {/if}
        <Button variant="secondary" onclick={() => groupAction((u) => bulkDelete($currentFolder, u), "Suppression impossible")}>Supprimer</Button>
        <Button variant="secondary" onclick={() => selection.set(new Set())}>Annuler</Button>
      </div>
    {/if}
    <MailList
      title={folder?.label ?? "Réception"} rows={visible} total={rows.length} bind:query bind:mode
      selectedUid={$selectedUid} selection={$selection}
      onopen={(uid) => selectedUid.set(uid)} onselectall={selectAll} ontoggle={toggle} onnew={compose}
    />
  </div>

  <MailReader {message} onback={() => selectedUid.set(null)} ondelete={removeCurrent} />
</div>

<style>
  .screen{display:grid;grid-template-columns:220px minmax(300px,380px) 1fr;height:100%;min-height:0;background:var(--bg)}
  aside{display:flex;flex-direction:column;gap:16px;padding:20px 8px;background:var(--sidebar);border-right:1px solid var(--border);overflow-y:auto}
  .title{padding:0 12px}
  .list{display:flex;flex-direction:column;min-height:0}
  .list :global(section){flex:1}
  .group{display:flex;flex-wrap:wrap;align-items:center;gap:8px;padding:8px 16px;background:var(--fill);border-right:1px solid var(--border);font-size:13px;color:var(--text-2)}
  .group span{margin-right:auto}
  @media (max-width:700px){
    .screen{grid-template-columns:1fr;grid-template-rows:auto 1fr}
    aside{flex-direction:row;align-items:center;padding:8px;border-right:0;border-bottom:1px solid var(--border);overflow-x:auto}
    .title{display:none}
    .screen.reading aside,.screen.reading .list{display:none}
    .screen:not(.reading) :global(article){display:none}
    .screen.reading{grid-template-rows:1fr}
  }
</style>

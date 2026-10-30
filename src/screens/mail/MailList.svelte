<!--
  MailList : barre de recherche, « Tout sélectionner », panneau Filtrer, liste groupée par jour.
  Props : title · rows (MessageRow[] déjà filtrées) · total (nombre avant filtre) · query (bindable) · mode (bindable)
  · selectedUid · selection (Set d'uid) · onopen(uid) · onselectall() · ontoggle(uid) · onnew().
  Ctrl/⌘+clic ajoute ou retire un message de la sélection ; Ctrl/⌘+A sélectionne tout (hors champ de saisie).
-->
<script lang="ts">
  import type { MessageRow } from "../../lib/types";
  import { Button, Chip } from "../../components/ui";
  import { FILTER_MODES, groupByDay, formatTime, type FilterMode } from "./helpers";

  let { title, rows, total, query = $bindable(""), mode = $bindable("all"), selectedUid, selection, onopen, onselectall, ontoggle, onnew }: {
    title: string; rows: MessageRow[]; total: number; query?: string; mode?: FilterMode;
    selectedUid: number | null; selection: Set<number>;
    onopen: (uid: number) => void; onselectall: () => void; ontoggle: (uid: number) => void; onnew: () => void;
  } = $props();

  let panelOpen = $state(false);
  const groups = $derived(groupByDay(rows));
  const allSelected = $derived(rows.length > 0 && rows.every((r) => selection.has(r.uid)));
  const modeLabel = $derived(FILTER_MODES.find((m) => m.mode === mode)?.label ?? "");
  const shortcut = typeof navigator !== "undefined" && /Mac/i.test(navigator.platform) ? "⌘A" : "Ctrl A";

  function pickMode(m: FilterMode) { mode = m; panelOpen = false; }

  function open(e: MouseEvent, uid: number) {
    if (e.ctrlKey || e.metaKey) ontoggle(uid);
    else onopen(uid);
  }

  function onkeydown(e: KeyboardEvent) {
    const t = e.target instanceof Element ? e.target : null;
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "a" && !t?.closest("input,textarea,iframe,[contenteditable]")) {
      e.preventDefault();
      onselectall();
    }
  }
</script>

<svelte:window {onkeydown} />

<section aria-label="Liste des messages">
  <header>
    <div class="head">
      <div>
        <h2>{title}</h2>
        <p class="sub">{total} {total > 1 ? "mails" : "mail"}</p>
      </div>
      <Button onclick={onnew}>Nouveau</Button>
    </div>
    <input type="search" placeholder="Rechercher" aria-label="Rechercher dans les mails" bind:value={query} />
    <div class="bar">
      <button type="button" class="tool" onclick={onselectall} aria-pressed={allSelected}>
        <span class="box" class:on={allSelected}></span>Tout sélectionner <kbd>{shortcut}</kbd>
      </button>
      <button type="button" class="tool filter" class:active={mode !== "all"} aria-expanded={panelOpen} onclick={() => (panelOpen = !panelOpen)}>
        {mode === "all" ? "Filtrer" : modeLabel}
      </button>
    </div>
    {#if panelOpen}
      <ul class="panel" role="listbox" aria-label="Filtrer la boîte">
        {#each FILTER_MODES as f (f.mode)}
          <li role="option" aria-selected={f.mode === mode}>
            <button type="button" onclick={() => pickMode(f.mode)}>
              <span>{f.label}</span>{#if f.mode === mode}<span aria-hidden="true">✓</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </header>

  <div class="rows">
    {#each groups as g (g.key)}
      <h3>{g.label}</h3>
      {#each g.items as m (m.uid)}
        <button type="button" class="row" class:open={m.uid === selectedUid} class:picked={selection.has(m.uid)}
          aria-pressed={selection.has(m.uid)} aria-current={m.uid === selectedUid ? "true" : undefined} onclick={(e) => open(e, m.uid)}>
          <span class="dot" class:unread={!m.seen} role={m.seen ? undefined : "img"} aria-label={m.seen ? undefined : "Non lu"}></span>
          <span class="body">
            <span class="line"><span class="from" class:strong={!m.seen}>{m.from_name || m.from_addr}</span><time>{formatTime(m.date)}</time></span>
            <span class="subject" class:strong={!m.seen}>{m.subject}</span>
            {#if m.sorted_into}<Chip>{m.sorted_into}</Chip>{/if}
          </span>
        </button>
      {/each}
    {:else}
      <p class="empty">{total === 0 ? "Aucun mail dans ce dossier." : "Aucun mail ne correspond."}</p>
    {/each}
  </div>
</section>

<style>
  section{display:flex;flex-direction:column;min-height:0;background:var(--bg);border-right:1px solid var(--border)}
  header{position:relative;padding:16px 16px 8px;display:flex;flex-direction:column;gap:10px}
  .head{display:flex;justify-content:space-between;align-items:flex-start;gap:12px}
  h2{margin:0;font:500 20px var(--font-ui)}
  .sub{margin:0;font:400 13px var(--font-ui);color:var(--text-3)}
  input[type=search]{min-height:var(--tap);background:var(--surface);color:var(--text);border:1px solid var(--border);padding:0 12px;font:400 15px var(--font-ui)}
  input::placeholder{color:var(--text-3)}
  input:focus-visible,.tool:focus-visible,.panel button:focus-visible,.row:focus-visible{outline:2px solid var(--accent-light);outline-offset:-2px}
  .bar{display:flex;justify-content:space-between;align-items:center}
  .tool{display:inline-flex;align-items:center;gap:8px;min-height:var(--tap);padding:0 8px;background:transparent;color:var(--text-2);border:0;font:400 13px var(--font-ui);cursor:pointer}
  .tool:hover{background:var(--fill)}
  .filter.active{background:var(--accent);color:var(--on-accent)}
  .box{width:14px;height:14px;border:1px solid var(--text-3);display:inline-block}
  .box.on{background:var(--accent-light);border-color:var(--accent-light)}
  kbd{font:400 11px var(--font-ui);color:var(--text-3)}
  .panel{position:absolute;right:16px;top:calc(100% - 8px);z-index:2;margin:0;padding:0;list-style:none;min-width:220px;background:var(--surface);border:1px solid var(--border)}
  .panel button{display:flex;justify-content:space-between;width:100%;min-height:var(--tap);padding:0 14px;background:transparent;color:var(--text);border:0;font:400 14px var(--font-ui);cursor:pointer;text-align:left}
  .panel button:hover{background:var(--fill)}
  .rows{overflow-y:auto;min-height:0;flex:1}
  h3{margin:0;padding:12px 16px 4px;font:400 12px var(--font-ui);color:var(--text-3)}
  .row{display:flex;gap:10px;width:100%;padding:12px 16px;background:transparent;color:var(--text-2);border:0;border-bottom:1px solid var(--border);text-align:left;cursor:pointer;font:400 13px var(--font-ui)}
  .row:hover{background:var(--surface)}
  .row.open{background:var(--accent);color:var(--on-accent)}
  .row.picked{background:var(--fill);outline:1px solid var(--accent-light);outline-offset:-1px}
  .dot{flex:none;width:8px;height:8px;margin-top:7px;background:transparent}
  .dot.unread{background:var(--accent-alt)}
  .open .dot.unread{background:var(--on-accent)}
  .body{display:flex;flex-direction:column;gap:4px;min-width:0;flex:1;align-items:flex-start}
  .line{display:flex;justify-content:space-between;gap:8px;width:100%}
  .from{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:var(--text)}
  .open .from,.open .subject{color:var(--on-accent)}
  time{flex:none;font-size:12px;color:var(--text-3)}
  .open time{color:var(--on-accent)}
  .subject{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;max-width:100%}
  .strong{font-weight:500}
  .empty{padding:24px 16px;color:var(--text-3);font:400 14px var(--font-ui)}
</style>

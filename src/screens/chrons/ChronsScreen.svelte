<!-- ChronsScreen : tâches planifiées, périodicité en clair, expression cron, prochain passage, interrupteur. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { Button, ScreenTitle, Toggle } from "../../components/ui";
  import { chronDelete, chronsList, chronToggle } from "../../lib/api";
  import { toast } from "../../lib/stores";
  import type { Chron } from "../../lib/types";
  import ChronEditor from "./ChronEditor.svelte";
  import { aboutKind, blankChron, nextRunLabel } from "./chronForm";

  let chrons = $state<Chron[]>([]);
  let loaded = $state(false);
  let editing = $state<Chron | null>(null);
  let confirming = $state<number | null>(null);

  const fail = (e: unknown) => toast.set({ kind: "error", text: String(e) });

  async function load() {
    try {
      chrons = await chronsList();
    } catch (e) {
      fail(e);
    }
    loaded = true;
  }
  onMount(load);

  async function toggle(c: Chron, enabled: boolean) {
    try {
      await chronToggle(c.id as number, enabled);
      await load();
    } catch (e) {
      fail(e);
      await load();
    }
  }

  async function remove(c: Chron) {
    try {
      await chronDelete(c.id as number);
      confirming = null;
      await load();
    } catch (e) {
      fail(e);
    }
  }

  async function saved() {
    editing = null;
    await load();
  }
</script>

<div class="screen">
  <header>
    <div>
      <ScreenTitle>Chrons</ScreenTitle>
      <p class="lead">Des tâches qui tournent seules, à l'heure que vous choisissez.</p>
    </div>
    <Button onclick={() => (editing = blankChron())}>Nouveau chron</Button>
  </header>

  <ul aria-label="Chrons">
    {#each chrons as c (c.id)}
      <li class:off={!c.enabled}>
        <div class="text">
          <strong>{c.name}</strong>
          <span class="about">{aboutKind(c.kind)}</span>
          <span class="when">{c.description} <code>{c.cron}</code></span>
          <span class="next">{nextRunLabel(c)}</span>
        </div>
        <div class="ctrl">
          <button type="button" class="link" onclick={() => (editing = { ...c })}>Modifier</button>
          {#if confirming === c.id}
            <button type="button" class="link" onclick={() => remove(c)}>Confirmer la suppression</button>
            <button type="button" class="link" onclick={() => (confirming = null)}>Garder</button>
          {:else}
            <button type="button" class="link" onclick={() => (confirming = c.id)}>Supprimer</button>
          {/if}
          <Toggle checked={c.enabled} label={`Activer le chron : ${c.name}`} onchange={(v) => toggle(c, v)} />
        </div>
      </li>
    {:else}
      {#if loaded}<li class="empty">Aucun chron pour l'instant.</li>{/if}
    {/each}
  </ul>

  {#if editing}
    {#key editing.id ?? "new"}
      <ChronEditor chron={editing} onsaved={saved} oncancel={() => (editing = null)} />
    {/key}
  {/if}
</div>

<style>
  .screen{padding:32px 40px;max-width:900px;overflow:auto;height:100%}
  header{display:flex;justify-content:space-between;align-items:flex-start;gap:16px;margin-bottom:24px}
  .lead{margin:6px 0 0;color:var(--text-2);font-size:14px}
  ul{list-style:none;margin:0;padding:0;display:flex;flex-direction:column;gap:1px;background:var(--border)}
  li{display:flex;justify-content:space-between;align-items:center;gap:16px;padding:16px;background:var(--surface)}
  li.off strong{color:var(--text-2)}
  .text{display:flex;flex-direction:column;gap:2px;min-width:0}
  .about,.next{color:var(--text-3);font-size:13px}
  .when{font-size:14px;color:var(--text-2)}
  code{font:400 13px ui-monospace,monospace;background:var(--fill);padding:2px 8px;margin-left:6px;color:var(--text)}
  .ctrl{display:flex;align-items:center;gap:4px;flex-shrink:0}
  .link{min-height:var(--tap);background:none;border:0;padding:0 8px;color:var(--accent-light);font:400 14px var(--font-ui);cursor:pointer}
  .link:focus-visible{outline:2px solid var(--accent-light)}
  .empty{color:var(--text-2)}
</style>

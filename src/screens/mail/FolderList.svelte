<!--
  FolderList : colonne des dossiers avec compteurs de non lus.
  Props : folders (Folder[]) · current (nom du dossier actif) · onselect (reçoit le nom du dossier).
  Dossiers système d'abord, puis « Triés pour vous » (dossiers créés par les règles).
-->
<script lang="ts">
  import type { Folder } from "../../lib/types";
  import { SectionLabel } from "../../components/ui";

  let { folders, current, onselect }: { folders: Folder[]; current: string; onselect: (name: string) => void } = $props();
  const system = $derived(folders.filter((f) => f.kind === "system"));
  const sorted = $derived(folders.filter((f) => f.kind === "sorted"));
</script>

{#snippet item(f: Folder)}
  <button type="button" class:active={f.name === current} aria-current={f.name === current ? "true" : undefined} onclick={() => onselect(f.name)}>
    <span class="name">{f.label}</span>
    {#if f.unread > 0}<span class="count" aria-label="{f.unread} non lus">{f.unread}</span>{/if}
  </button>
{/snippet}

<nav aria-label="Dossiers">
  {#each system as f (f.name)}{@render item(f)}{/each}
  {#if sorted.length > 0}
    <div class="label"><SectionLabel>Triés pour vous</SectionLabel></div>
    {#each sorted as f (f.name)}{@render item(f)}{/each}
  {/if}
</nav>

<style>
  nav{display:flex;flex-direction:column;gap:2px}
  button{display:flex;align-items:center;justify-content:space-between;gap:8px;min-height:var(--tap);padding:0 12px;background:transparent;color:var(--text-2);border:0;font:400 15px var(--font-ui);text-align:left;cursor:pointer}
  button:hover{background:var(--fill)}
  button.active{background:var(--accent);color:var(--on-accent)}
  button:focus-visible{outline:2px solid var(--accent-light);outline-offset:-2px}
  .count{font:500 12px var(--font-ui);color:var(--text-3)}
  .active .count{color:var(--on-accent)}
  .label{padding:16px 12px 6px}
  @media (max-width:700px){
    nav{flex-direction:row;overflow-x:auto;gap:4px}
    .label{display:none}
    button{flex:none}
  }
</style>

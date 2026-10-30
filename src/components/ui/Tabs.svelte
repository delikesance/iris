<!--
  Tabs : onglets. Props : items ({id:string,label:string}[]) · active (string, bindable) · onchange (handler, reçoit l'id).
  Onglet actif souligné par --accent-light ; hauteur >= --tap.
-->
<script lang="ts">
  let { items, active = $bindable(""), onchange }: {
    items: { id: string; label: string }[]; active?: string; onchange?: (id: string) => void;
  } = $props();
  function pick(id: string) { active = id; onchange?.(id); }
</script>

<div role="tablist">
  {#each items as it (it.id)}
    <button type="button" role="tab" aria-selected={active === it.id} class:on={active === it.id} onclick={() => pick(it.id)}>{it.label}</button>
  {/each}
</div>

<style>
  div{display:flex;gap:4px;border-bottom:1px solid var(--border)}
  button{min-height:var(--tap);padding:0 16px;background:none;border:0;border-bottom:2px solid transparent;color:var(--text-3);font:500 14px var(--font-ui);cursor:pointer}
  button.on{color:var(--text);border-bottom-color:var(--accent-light)}
  button:focus-visible{outline:2px solid var(--accent-light)}
</style>

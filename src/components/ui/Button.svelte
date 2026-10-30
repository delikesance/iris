<!--
  Button. Props : variant ("primary" fond --accent texte blanc | "secondary" bordure seule, défaut primary)
  · type ("button" | "submit", défaut button) · disabled (bool) · onclick (handler) · children (contenu).
  Hauteur minimale --tap (44px), angles droits.
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  let { variant = "primary", type = "button", disabled = false, onclick, children }: {
    variant?: "primary" | "secondary"; type?: "button" | "submit"; disabled?: boolean;
    onclick?: (e: MouseEvent) => void; children: Snippet;
  } = $props();
</script>

<button class={variant} {type} {disabled} {onclick}>{@render children()}</button>

<style>
  button{min-height:var(--tap);padding:0 20px;font:500 15px var(--font-ui);cursor:pointer;border:1px solid transparent}
  .primary{background:var(--accent);color:var(--on-accent)}
  .primary:hover:not(:disabled){background:var(--accent-alt)}
  .secondary{background:transparent;color:var(--text);border-color:var(--border)}
  .secondary:hover:not(:disabled){background:var(--fill)}
  button:focus-visible{outline:2px solid var(--accent-light);outline-offset:2px}
  button:disabled{opacity:.5;cursor:default}
</style>

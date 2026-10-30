<!--
  Chip : étiquette compacte (dossier, pièce jointe, état). Props : children (contenu).
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  let { children }: { children: Snippet } = $props();
</script>

<span>{@render children()}</span>

<style>
  span{display:inline-flex;align-items:center;min-height:24px;padding:0 10px;background:var(--fill);color:var(--text-2);font:400 12px var(--font-ui)}
</style>

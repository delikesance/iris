<!--
  SectionLabel : petit libellé de section (Inter 12 px, majuscules, interlettrage 0,12 em).
  Props : children (contenu).
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  let { children }: { children: Snippet } = $props();
</script>

<h3>{@render children()}</h3>

<style>
  h3{margin:0;font:500 12px var(--font-ui);text-transform:uppercase;letter-spacing:.12em;color:var(--accent-light)}
</style>

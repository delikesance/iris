<!--
  ScreenTitle : titre d'écran (Barlow Condensed Light 300). Props : children (contenu) · size (px, défaut 44).
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  let { children, size = 44 }: { children: Snippet; size?: number } = $props();
</script>

<h1 style:font-size="{size}px">{@render children()}</h1>

<style>
  h1{margin:0;font-family:var(--font-title);font-weight:300;line-height:1.1;color:var(--text)}
</style>

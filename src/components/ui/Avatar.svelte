<!--
  Avatar : carré à initiales. Props : name (string, initiales déduites) · variant ("blue" | "gray" | "me", défaut gray)
  · size (px, défaut 32).
-->
<script lang="ts">
  let { name, variant = "gray", size = 32 }: { name: string; variant?: "blue" | "gray" | "me"; size?: number } = $props();
  const initials = $derived(name.split(/[\s@.]+/).filter(Boolean).slice(0, 2).map(p => p[0].toUpperCase()).join(""));
</script>

<span class={variant} style:width="{size}px" style:height="{size}px" style:font-size="{Math.round(size * 0.38)}px" title={name}>{initials}</span>

<style>
  span{display:inline-flex;align-items:center;justify-content:center;font-family:var(--font-ui);font-weight:500;flex:none}
  .blue{background:var(--avatar-blue-bg);color:var(--avatar-blue-fg)}
  .gray{background:var(--avatar-gray-bg);color:var(--avatar-gray-fg)}
  .me{background:var(--avatar-me-bg);color:var(--text)}
</style>

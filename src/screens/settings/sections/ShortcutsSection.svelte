<!-- ShortcutsSection : raccourcis clavier disponibles. -->
<script lang="ts">
  import { SectionLabel } from "../../../components/ui";
  const shortcuts = [{ keys: "Ctrl / Cmd + A", does: "Tout sélectionner dans la liste des messages" }];
</script>

<SectionLabel>Raccourcis clavier</SectionLabel>
<dl>
  {#each shortcuts as s (s.keys)}<dt><kbd>{s.keys}</kbd></dt><dd>{s.does}</dd>{/each}
</dl>

<style>
  dl{display:grid;grid-template-columns:max-content 1fr;gap:8px 24px;margin:16px 0}
  dd{margin:0;color:var(--text-2)}
  kbd{font:400 13px ui-monospace,monospace;background:var(--fill);padding:2px 8px}
</style>

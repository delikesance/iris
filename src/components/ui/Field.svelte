<!--
  Field : champ texte avec libellé. Props : label (string) · value (string, bindable) · type (défaut "text")
  · placeholder · name · disabled · multiline (bool, textarea) · rows (défaut 6, si multiline).
  Fond --surface, bordure --border, hauteur >= --tap.
-->
<script lang="ts">
  let { label, value = $bindable(""), type = "text", placeholder = "", name, disabled = false, multiline = false, rows = 6 }: {
    label: string; value?: string; type?: string; placeholder?: string; name?: string;
    disabled?: boolean; multiline?: boolean; rows?: number;
  } = $props();
</script>

<label>
  <span>{label}</span>
  {#if multiline}
    <textarea {name} {placeholder} {disabled} {rows} bind:value></textarea>
  {:else}
    <input {name} {type} {placeholder} {disabled} bind:value />
  {/if}
</label>

<style>
  label{display:flex;flex-direction:column;gap:6px}
  span{font:400 13px var(--font-ui);color:var(--text-3)}
  input,textarea{background:var(--surface);color:var(--text);border:1px solid var(--border);padding:0 12px;font:400 15px var(--font-ui)}
  input{min-height:var(--tap)}
  textarea{padding:10px 12px;resize:vertical}
  input:focus,textarea:focus{outline:2px solid var(--accent-light);outline-offset:0}
  ::placeholder{color:var(--text-3)}
</style>

<!--
  Toggle : interrupteur carré, curseur --text. Props : checked (bool, bindable) · label (string, nom accessible)
  · disabled · onchange (handler, reçoit le nouvel état). Zone cliquable >= --tap.
-->
<script lang="ts">
  let { checked = $bindable(false), label, disabled = false, onchange }: {
    checked?: boolean; label: string; disabled?: boolean; onchange?: (v: boolean) => void;
  } = $props();
  function flip() { checked = !checked; onchange?.(checked); }
</script>

<button type="button" role="switch" aria-checked={checked} aria-label={label} {disabled} onclick={flip}>
  <span class="track" class:on={checked}><span class="knob"></span></span>
</button>

<style>
  button{min-width:var(--tap);min-height:var(--tap);background:none;border:0;padding:0;cursor:pointer;display:inline-flex;align-items:center;justify-content:center}
  .track{width:40px;height:20px;background:var(--fill);border:1px solid var(--border);position:relative}
  .track.on{background:var(--accent);border-color:var(--accent)}
  .knob{position:absolute;top:2px;left:2px;width:14px;height:14px;background:var(--text)}
  .on .knob{left:22px}
  button:focus-visible{outline:2px solid var(--accent-light)}
  button:disabled{opacity:.5;cursor:default}
</style>

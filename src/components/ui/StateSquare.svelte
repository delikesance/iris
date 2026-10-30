<!--
  StateSquare : carré plein + libellé. L'état se lit au libellé, jamais à la couleur seule.
  Props : state ("todo" | "wait" | "done") · label (string, défaut : À signer / En attente / Signé).
-->
<script lang="ts">
  type State = "todo" | "wait" | "done";
  const defaults: Record<State, string> = { todo: "À signer", wait: "En attente", done: "Signé" };
  let { state, label }: { state: State; label?: string } = $props();
</script>

<span class="s"><i class={state}></i>{label ?? defaults[state]}</span>

<style>
  .s{display:inline-flex;align-items:center;gap:8px;font:400 13px var(--font-ui);color:var(--text-2)}
  i{width:10px;height:10px;display:inline-block}
  .todo{background:var(--state-todo)}
  .wait{background:var(--state-wait)}
  .done{background:var(--state-done)}
</style>

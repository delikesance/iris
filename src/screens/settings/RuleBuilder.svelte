<!--
  RuleBuilder : constructeur Si / Alors avec phrase en direct (commande rule_sentence).
  Props : rule (Rule | null, null = nouvelle règle) · nextPosition (number) · onsaved (handler, reçoit la règle
  enregistrée) · oncancel (handler).
-->
<script lang="ts">
  import { untrack } from "svelte";
  import { Button, Field, SectionLabel } from "../../components/ui";
  import { ruleSave, ruleSentence } from "../../lib/api";
  import type { Action, Cond, Rule } from "../../lib/types";
  import { liveCheck } from "./live";
  import {
    actionKinds, blankItem, condKinds, isComplete, leaves, paramValue, toCond, withParam,
  } from "./ruleBuilder";

  let { rule, nextPosition, onsaved, oncancel }: {
    rule: Rule | null; nextPosition: number; onsaved: (r: Rule) => void; oncancel: () => void;
  } = $props();

  let conds = $state<Cond[]>(untrack(() => (rule ? leaves(rule.cond) : [blankItem<Cond>(condKinds, "sender_is")])));
  let action = $state<Action>(untrack(() => rule?.action ?? blankItem<Action>(actionKinds, "move_to")));
  let sentence = $state("");
  let error = $state("");
  let saving = $state(false);

  const ready = $derived(conds.every((c) => isComplete(condKinds, c)) && isComplete(actionKinds, action));
  const live = liveCheck(
    ([c, a]: [Cond, Action]) => ruleSentence(c, a),
    (s) => { sentence = s; error = ""; },
    (e) => { sentence = ""; error = e; },
  );

  $effect(() => {
    if (ready) live.run([toCond($state.snapshot(conds) as Cond[]), $state.snapshot(action) as Action]);
    else { live.cancel(); sentence = ""; error = ""; }
    return live.cancel;
  });

  function cond(i: number, fn: (c: Cond) => Cond) {
    conds[i] = fn(conds[i]);
  }

  async function save() {
    saving = true;
    try {
      onsaved(await ruleSave({
        id: rule?.id ?? null, name: sentence, enabled: rule?.enabled ?? true,
        position: rule?.position ?? nextPosition, cond: toCond($state.snapshot(conds) as Cond[]),
        action: $state.snapshot(action) as Action,
      }));
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<section class="builder" aria-label={rule ? "Modifier la règle" : "Nouvelle règle"}>
  <SectionLabel>{rule ? "Modifier la règle" : "Nouvelle règle"}</SectionLabel>

  <div class="block">
    <span class="word">Si</span>
    {#each conds as c, i (i)}
      <div class="row">
        <select aria-label="Condition" value={c.type} onchange={(e) => cond(i, () => blankItem<Cond>(condKinds, e.currentTarget.value))}>
          {#each condKinds as k (k.type)}<option value={k.type}>{k.label}</option>{/each}
        </select>
        {#if condKinds.find((k) => k.type === c.type)?.param}
          {@const p = condKinds.find((k) => k.type === c.type)!.param!}
          <input aria-label="Valeur de la condition" type={p.kind === "number" ? "number" : "text"} min="1"
            placeholder={p.placeholder} value={paramValue(condKinds, c)}
            oninput={(e) => cond(i, (x) => withParam(condKinds, x, e.currentTarget.value))} />
        {/if}
        {#if conds.length > 1}
          <button type="button" class="link" onclick={() => conds.splice(i, 1)}>Retirer</button>
        {/if}
      </div>
    {/each}
    <button type="button" class="link" onclick={() => conds.push(blankItem<Cond>(condKinds, "sender_is"))}>+ Ajouter une condition</button>
  </div>

  <div class="block">
    <span class="word">Alors</span>
    <div class="row">
      <select aria-label="Action" value={action.type} onchange={(e) => (action = blankItem<Action>(actionKinds, e.currentTarget.value))}>
        {#each actionKinds as k (k.type)}<option value={k.type}>{k.label}</option>{/each}
      </select>
      {#if actionKinds.find((k) => k.type === action.type)?.param}
        {@const p = actionKinds.find((k) => k.type === action.type)!.param!}
        <input aria-label="Valeur de l'action" type={p.kind === "number" ? "number" : "text"} min="1"
          placeholder={p.placeholder} value={paramValue(actionKinds, action)}
          oninput={(e) => (action = withParam(actionKinds, action, e.currentTarget.value))} />
      {/if}
    </div>
  </div>

  <p class="sentence" aria-live="polite" data-testid="rule-sentence">
    {#if sentence}{sentence}{:else}<span class="hint">La phrase de la règle apparaît ici.</span>{/if}
  </p>
  {#if error}<p class="error" role="alert">{error}</p>{/if}

  <div class="actions">
    <Button variant="secondary" onclick={oncancel}>Annuler</Button>
    <Button onclick={save} disabled={!ready || !sentence || saving}>Enregistrer la règle</Button>
  </div>
</section>

<style>
  .builder{display:flex;flex-direction:column;gap:16px;background:var(--surface);border:1px solid var(--border);padding:20px}
  .block{display:flex;flex-direction:column;gap:8px}
  .word{font:500 12px var(--font-ui);color:var(--text-3);text-transform:uppercase;letter-spacing:.12em}
  .row{display:flex;gap:8px;flex-wrap:wrap;align-items:center}
  select,input{min-height:var(--tap);background:var(--bg);color:var(--text);border:1px solid var(--border);padding:0 12px;font:400 15px var(--font-ui)}
  input{min-width:220px;flex:1}
  select:focus-visible,input:focus-visible{outline:2px solid var(--accent-light)}
  ::placeholder{color:var(--text-3)}
  .link{min-height:var(--tap);background:none;border:0;padding:0 4px;color:var(--accent-light);font:400 14px var(--font-ui);cursor:pointer;align-self:flex-start}
  .link:focus-visible{outline:2px solid var(--accent-light)}
  .sentence{margin:0;padding:12px 14px;background:var(--fill);color:var(--text);min-height:48px}
  .hint{color:var(--text-3)}
  .error{margin:0;color:var(--text-2);border-left:2px solid var(--accent-light);padding-left:10px}
  .actions{display:flex;justify-content:flex-end;gap:12px}
</style>

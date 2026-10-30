<!--
  RulesSection : liste des règles écrites en phrases (Toggle) + constructeur.
  Props : editId (number | null, règle ouverte par le lien profond settings/rules/<id>).
  L'édition passe par la route : Modifier -> settings/rules/<id>, fermer -> settings.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { Button, Toggle } from "../../components/ui";
  import { ruleDelete, rulesList, ruleSentence, ruleToggle } from "../../lib/api";
  import { route, toast } from "../../lib/stores";
  import type { Rule } from "../../lib/types";
  import RuleBuilder from "./RuleBuilder.svelte";
  import { ruleRoute } from "./ruleBuilder";

  let { editId = null }: { editId?: number | null } = $props();

  let rules = $state<Rule[]>([]);
  let sentences = $state<Record<number, string>>({});
  let loaded = $state(false);
  let creating = $state(false);
  let confirming = $state<number | null>(null);

  const editing = $derived(editId == null ? null : (rules.find((r) => r.id === editId) ?? null));
  const showBuilder = $derived(creating || editing != null);
  const fail = (e: unknown) => toast.set({ kind: "error", text: String(e) });

  async function load() {
    try {
      rules = await rulesList();
      const pairs = await Promise.all(rules.map(async (r) => [r.id as number, await ruleSentence(r.cond, r.action).catch(() => r.name)] as const));
      sentences = Object.fromEntries(pairs);
    } catch (e) {
      fail(e);
    }
    loaded = true;
  }
  onMount(load);

  async function toggle(r: Rule, enabled: boolean) {
    try {
      await ruleToggle(r.id as number, enabled);
      r.enabled = enabled;
    } catch (e) {
      fail(e);
      await load();
    }
  }

  async function remove(r: Rule) {
    try {
      await ruleDelete(r.id as number);
      confirming = null;
      await load();
    } catch (e) {
      fail(e);
    }
  }

  async function saved() {
    creating = false;
    route.set("settings");
    await load();
  }

  function close() {
    creating = false;
    route.set("settings");
  }
</script>

<div class="head">
  <p class="lead">Chaque message est classé à son arrivée. Rien n'est jamais supprimé.</p>
  <Button onclick={() => { route.set("settings"); creating = true; }}>Nouvelle règle</Button>
</div>

{#if loaded && editId != null && !editing && !creating}
  <p class="missing" role="alert">Cette règle n'existe plus.</p>
{/if}

<ul class="rules" aria-label="Règles de tri">
  {#each rules as r (r.id)}
    <li class:off={!r.enabled}>
      <div class="text">
        <span class="sentence">{sentences[r.id as number] ?? r.name}</span>
        {#if !r.enabled}<span class="state">Désactivée</span>{/if}
      </div>
      <div class="ctrl">
        <button type="button" class="link" onclick={() => { creating = false; route.set(ruleRoute(r.id as number)); }}>Modifier</button>
        {#if confirming === r.id}
          <button type="button" class="link" onclick={() => remove(r)}>Confirmer la suppression</button>
          <button type="button" class="link" onclick={() => (confirming = null)}>Garder</button>
        {:else}
          <button type="button" class="link" onclick={() => (confirming = r.id)}>Supprimer</button>
        {/if}
        <Toggle checked={r.enabled} label={`Activer la règle : ${sentences[r.id as number] ?? r.name}`} onchange={(v) => toggle(r, v)} />
      </div>
    </li>
  {:else}
    {#if loaded}<li class="empty">Aucune règle pour l'instant.</li>{/if}
  {/each}
</ul>

{#if showBuilder}
  {#key editing?.id ?? "new"}
    <RuleBuilder rule={creating ? null : editing} nextPosition={rules.length} onsaved={saved} oncancel={close} />
  {/key}
{/if}

<style>
  .head{display:flex;justify-content:space-between;align-items:center;gap:16px;margin-bottom:16px}
  .lead{margin:0;color:var(--text-2);font-size:14px}
  .rules{list-style:none;margin:0 0 24px;padding:0;display:flex;flex-direction:column;gap:1px;background:var(--border)}
  li{display:flex;justify-content:space-between;align-items:center;gap:16px;padding:12px 16px;background:var(--surface)}
  li.off .sentence{color:var(--text-2)}
  .text{display:flex;flex-direction:column;min-width:0}
  .state{font-size:13px;color:var(--text-3)}
  .ctrl{display:flex;align-items:center;gap:4px;flex-shrink:0}
  .link{min-height:var(--tap);background:none;border:0;padding:0 8px;color:var(--accent-light);font:400 14px var(--font-ui);cursor:pointer}
  .link:focus-visible{outline:2px solid var(--accent-light)}
  .empty,.missing{color:var(--text-2)}
  .missing{margin:0 0 16px;border-left:2px solid var(--accent-light);padding-left:10px}
</style>

<!--
  ChronEditor : création / modification d'un chron. L'expression est validée en direct (cron_describe) :
  périodicité en clair et 3 prochains passages, ou message d'erreur.
  Props : chron (Chron, brouillon ou existant) · onsaved (handler, reçoit le chron enregistré) · oncancel (handler).
-->
<script lang="ts">
  import { untrack } from "svelte";
  import { Button, Field, SectionLabel } from "../../components/ui";
  import { chronSave, cronDescribe } from "../../lib/api";
  import type { Chron, ChronKind, CronDescription } from "../../lib/types";
  import { liveCheck } from "../settings/live";
  import { aboutKind, canSave, isoLabel, kindOptions } from "./chronForm";

  let { chron, onsaved, oncancel }: { chron: Chron; onsaved: (c: Chron) => void; oncancel: () => void } = $props();

  const start = untrack(() => chron);
  let name = $state(start.name);
  let cron = $state(start.cron);
  let kind = $state<ChronKind>(start.kind);
  let check = $state<CronDescription | null>(null);
  let error = $state("");
  let saving = $state(false);

  const live = liveCheck(
    (expr: string) => cronDescribe(expr),
    (d) => { check = d; },
    (e) => { check = { ok: false, description: null, next_runs: [], error: e }; },
    200,
  );

  $effect(() => {
    if (cron.trim()) live.run(cron.trim());
    else { live.cancel(); check = null; }
    return live.cancel;
  });

  async function save() {
    saving = true;
    error = "";
    try {
      onsaved(await chronSave({ ...start, name: name.trim(), cron: cron.trim(), kind }));
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<section class="editor" aria-label={start.id == null ? "Nouveau chron" : "Modifier le chron"}>
  <SectionLabel>{start.id == null ? "Nouveau chron" : "Modifier le chron"}</SectionLabel>
  <Field label="Nom" bind:value={name} placeholder="Résumé du matin" />
  <label class="kind">
    <span>Tâche</span>
    <select bind:value={kind}>
      {#each kindOptions as o (o.kind)}<option value={o.kind}>{o.label}</option>{/each}
    </select>
    <small>{aboutKind(kind)}</small>
  </label>
  <Field label="Expression cron (minute heure jour mois jour-de-semaine)" bind:value={cron} placeholder="0 8 * * 1-5" />

  <div class="check" aria-live="polite" data-testid="cron-check">
    {#if check?.ok}
      <p class="desc">{check.description}</p>
      <ul>{#each check.next_runs as iso (iso)}<li>{isoLabel(iso)}</li>{/each}</ul>
    {:else if check}
      <p class="bad" role="alert">{check.error ?? "Expression invalide."}</p>
    {/if}
  </div>
  {#if error}<p class="bad" role="alert">{error}</p>{/if}

  <div class="actions">
    <Button variant="secondary" onclick={oncancel}>Annuler</Button>
    <Button onclick={save} disabled={!canSave(name, check?.ok === true) || saving}>Enregistrer</Button>
  </div>
</section>

<style>
  .editor{display:flex;flex-direction:column;gap:16px;background:var(--surface);border:1px solid var(--border);padding:20px;margin-top:24px}
  .kind{display:flex;flex-direction:column;gap:6px}
  .kind span{font-size:13px;color:var(--text-3)}
  select{min-height:var(--tap);background:var(--bg);color:var(--text);border:1px solid var(--border);padding:0 12px;font:400 15px var(--font-ui)}
  select:focus-visible{outline:2px solid var(--accent-light)}
  small{color:var(--text-3);font-size:13px}
  .check{min-height:24px}
  .desc{margin:0;color:var(--text)}
  ul{margin:8px 0 0;padding-left:18px;color:var(--text-2);font-size:14px}
  .bad{margin:0;color:var(--text-2);border-left:2px solid var(--accent-light);padding-left:10px}
  .actions{display:flex;justify-content:flex-end;gap:12px}
</style>

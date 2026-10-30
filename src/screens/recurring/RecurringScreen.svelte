<script lang="ts">
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { Avatar, Button, Chip, ScreenTitle, SectionLabel, Tabs } from "../../components/ui";
  import { newSendersList, recurringList, senderDecide, unsubscribe } from "../../lib/api";
  import { toast } from "../../lib/stores";
  import type { NewSender, RecurringSender, SenderDecision } from "../../lib/types";
  import { KIND_LABEL, groupRows, tabItems, unreadLabel, type Tab } from "./recurring";

  let newSenders = $state<NewSender[]>([]);
  let rows = $state<RecurringSender[]>([]);
  let tab = $state<Tab>("all");
  let confirming = $state<string | null>(null);
  const groups = $derived(groupRows(rows, tab));

  const fail = (e: unknown) => toast.set({ kind: "error", text: String(e) });

  async function load() {
    try {
      [newSenders, rows] = await Promise.all([newSendersList(), recurringList()]);
    } catch (e) { fail(e); }
  }

  async function decide(s: NewSender, decision: SenderDecision) {
    try {
      await senderDecide(s.addr, decision);
      await load();
    } catch (e) { fail(e); }
  }

  async function unsub(r: RecurringSender) {
    confirming = null;
    try {
      const res = await unsubscribe(r.addr);
      if (res.open_url) await openUrl(res.open_url);
      toast.set({ kind: "info", text: res.done ? `Désinscription envoyée à ${r.name}.` : `Page de désinscription de ${r.name} ouverte.` });
      await load();
    } catch (e) { fail(e); }
  }

  onMount(load);
</script>

<section class="screen">
  <ScreenTitle>Récurrents</ScreenTitle>
  <p class="lead">Les expéditeurs qui t'écrivent régulièrement, regroupés pour alléger ta réception.</p>

  {#if newSenders.length > 0}
    <div class="panel" data-testid="new-senders">
      <SectionLabel>Nouveaux expéditeurs</SectionLabel>
      <p class="sub">Choisis où leurs mails doivent arriver.</p>
      {#each newSenders as s (s.addr)}
        <div class="row">
          <div class="who"><strong>{s.name}</strong><span>{s.last_subject}</span></div>
          <Button variant="secondary" onclick={() => decide(s, "block")}>Bloquer</Button>
          <Button variant="secondary" onclick={() => decide(s, "recurring")}>Vers Récurrents</Button>
          <Button onclick={() => decide(s, "inbox")}>Vers Réception</Button>
        </div>
      {/each}
    </div>
  {/if}

  <Tabs items={tabItems(rows)} bind:active={tab} />

  {#each groups as g (g.kind)}
    <SectionLabel>{KIND_LABEL[g.kind]}</SectionLabel>
    <div class="panel">
      {#each g.rows as r (r.addr)}
        <div class="row" data-testid="recurring-row">
          <Avatar name={r.name} />
          <div class="who">
            <strong>{r.name} <small>{r.cadence}</small></strong>
            <span>{r.last_subject}</span>
          </div>
          {#if r.unread > 0}<span class="unread">{unreadLabel(r.unread)}</span>{/if}
          {#if r.filed_in}
            <Chip>Classé dans {r.filed_in}</Chip>
          {:else if r.can_unsubscribe}
            {#if confirming === r.addr}
              <Button onclick={() => unsub(r)}>Confirmer</Button>
              <Button variant="secondary" onclick={() => (confirming = null)}>Annuler</Button>
            {:else}
              <Button variant="secondary" onclick={() => (confirming = r.addr)}>Se désinscrire</Button>
            {/if}
          {/if}
        </div>
      {/each}
    </div>
  {:else}
    <p class="sub">Aucun expéditeur récurrent pour l'instant.</p>
  {/each}
</section>

<style>
  .screen{display:flex;flex-direction:column;gap:16px;padding:32px 40px;overflow:auto;height:100%;box-sizing:border-box}
  .lead,.sub{margin:0;color:var(--text-3);font:400 14px var(--font-ui)}
  .panel{display:flex;flex-direction:column;background:var(--surface);padding:8px 16px}
  .panel > :global(h3){margin-top:8px}
  .row{display:flex;align-items:center;gap:12px;padding:10px 0;border-top:1px solid var(--border);min-height:var(--tap)}
  .row:first-of-type{border-top:0}
  .who{flex:1;display:flex;flex-direction:column;gap:2px;min-width:0}
  .who strong{font:500 14px var(--font-ui);color:var(--text)}
  .who small{font:400 12px var(--font-ui);color:var(--text-3);margin-left:6px}
  .who span{font:400 13px var(--font-ui);color:var(--text-2);overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .unread{font:400 12px var(--font-ui);color:var(--accent-light)}
</style>

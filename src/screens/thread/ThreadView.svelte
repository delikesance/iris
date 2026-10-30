<!--
  ThreadView : fil de discussion (PDF p.13). Props : threadId (string) · reminderDays (number | null, relance en
  attente posée à l'envoi ; le contrat thread_get ne la fournit pas) · onclose (retour à la liste, optionnel).
-->
<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import * as api from "../../lib/api";
  import { composeDraft, signingTarget, toast } from "../../lib/stores";
  import type { Attachment, ThreadMessage } from "../../lib/types";
  import SigningPanel from "../../components/signing/SigningPanel.svelte";
  import { Avatar, Button, Chip } from "../../components/ui";
  import {
    canSign, defaultOpen, groupByDay, isSuperseded, legend, participants, pendingReminder,
    replyAllDraft, splitQuoted, supersededLabel, timeLabel,
  } from "./helpers";

  let { threadId, reminderDays = null, onclose }: {
    threadId: string; reminderDays?: number | null; onclose?: () => void;
  } = $props();

  let messages = $state<ThreadMessage[]>([]);
  let me = $state("");
  let error = $state("");
  let loading = $state(true);
  let open = $state(new Set<number>());
  let quoted = $state(new Set<number>());

  const ps = $derived(participants(messages, me));
  const groups = $derived(groupByDay(messages));
  const pill = $derived(pendingReminder(messages, me, reminderDays));
  const title = $derived(messages[0]?.subject ?? "");
  const variant = (addr: string) => {
    const p = ps.find(x => x.addr === addr.toLowerCase());
    return p?.me ? "me" : ps.indexOf(p!) % 2 === 0 ? "blue" : "gray";
  };

  $effect(() => {
    const id = threadId;
    loading = true; error = "";
    Promise.all([api.threadGet(id), api.account()])
      .then(([list, account]) => { messages = list; me = account; open = defaultOpen(list); })
      .catch(e => (error = String(e)))
      .finally(() => (loading = false));
  });

  function toggle(set: Set<number>, uid: number) {
    const next = new Set(set);
    if (!next.delete(uid)) next.add(uid);
    return next;
  }

  async function download(m: ThreadMessage, a: Attachment) {
    const dest = await save({ defaultPath: a.filename });
    if (!dest) return;
    try { await api.saveAttachment(m.folder, m.uid, a.index, dest); toast.set({ kind: "info", text: `${a.filename} enregistré.` }); }
    catch (e) { toast.set({ kind: "error", text: String(e) }); }
  }

  function sign(m: ThreadMessage, a: Attachment) {
    signingTarget.set({ folder: m.folder, uid: m.uid, index: a.index });
  }

  const signed = $derived.by(() => {
    const t = $signingTarget;
    const m = t && messages.find((x) => x.folder === t.folder && x.uid === t.uid);
    return t && m ? { t, m } : null;
  });

  const replyAll = () => composeDraft.set(replyAllDraft(messages, me));
</script>

<section class="thread">
  {#if onclose}<button type="button" class="back" onclick={onclose}>← Retour</button>{/if}
  {#if loading}
    <p class="state">Chargement…</p>
  {:else if error}
    <p class="state" role="alert">{error}</p>
  {:else if messages.length}
    <h1>{title}</h1>
    <div class="legend">
      <span>{legend(messages, ps)}</span>
      {#each ps as p (p.addr)}
        <span class="who"><Avatar name={p.name} variant={variant(p.addr)} size={20} />{p.name} · {p.count}</span>
      {/each}
    </div>

    <div class="timeline">
      {#each groups as g (g.label)}
        <div class="day">{g.label}</div>
        {#each g.items as m (m.uid)}
          {@const mine = m.from_addr.toLowerCase() === me.toLowerCase()}
          {@const parts = splitQuoted(m.text)}
          <article>
            <Avatar name={mine ? "Vous" : (m.from_name || m.from_addr)} variant={variant(m.from_addr)} size={28} />
            <div class="content">
              <button type="button" class="head" aria-expanded={open.has(m.uid)} onclick={() => (open = toggle(open, m.uid))}>
                <strong>{mine ? "Vous" : (m.from_name || m.from_addr)}</strong>
                <span class="to">à {m.to}</span>
                <time>{timeLabel(m.date)}</time>
              </button>
              {#if open.has(m.uid)}
                <div class="bubble" class:mine>
                  <div class="text">{parts.body}</div>
                  {#each m.attachments as a (a.index)}
                    <div class="att">
                      <button type="button" class="chip" onclick={() => download(m, a)}>{a.filename}</button>
                      {#if isSuperseded(a, m)}
                        <Chip>{supersededLabel(a.superseded_by)}</Chip>
                      {:else if canSign(a, m)}
                        <Button variant="secondary" onclick={() => sign(m, a)}>Signer</Button>
                      {/if}
                    </div>
                  {/each}
                </div>
                {#if parts.quoted}
                  <button type="button" class="link" onclick={() => (quoted = toggle(quoted, m.uid))}>
                    {quoted.has(m.uid) ? "Masquer le message cité" : "Afficher le message cité"}
                  </button>
                  {#if quoted.has(m.uid)}<div class="quote">{parts.quoted}</div>{/if}
                {/if}
              {/if}
            </div>
          </article>
        {/each}
      {/each}
    </div>

    {#if signed}
      {#key `${signed.t.folder}/${signed.t.uid}/${signed.t.index}`}
        <SigningPanel
          target={signed.t} filename={signed.m.attachments.find((a) => a.index === signed.t.index)?.filename ?? ""}
          replyTo={signed.m.from_addr} subject={signed.m.subject} inReplyTo={signed.m.message_id}
          onclose={() => signingTarget.set(null)}
        />
      {/key}
    {/if}

    <footer>
      {#if pill}<span class="pill">{pill}</span>{/if}
      <Button onclick={replyAll}>Répondre à tous</Button>
    </footer>
  {/if}
</section>

<style>
  .thread{padding:24px 32px;max-width:860px}
  h1{margin:0 0 8px;font:300 32px/1.1 var(--font-title)}
  .state{color:var(--text-2)}
  .back{min-height:var(--tap);background:none;border:0;color:var(--accent-light);cursor:pointer;font:400 13px var(--font-ui);padding:0}
  .legend{display:flex;flex-wrap:wrap;gap:8px 16px;align-items:center;font-size:12px;color:var(--text-2);margin-bottom:16px}
  .who{display:inline-flex;align-items:center;gap:6px}
  .timeline{position:relative;border-top:1px solid var(--border)}
  .timeline::before{content:"";position:absolute;left:13px;top:0;bottom:0;width:1px;background:var(--border)}
  .day{position:relative;font-size:12px;color:var(--text-3);padding:12px 0 8px;background:var(--bg);display:inline-block}
  article{position:relative;display:flex;gap:12px;padding:6px 0 10px}
  article :global(span:first-child){position:relative}
  .content{flex:1;min-width:0}
  .head{display:flex;gap:8px;align-items:baseline;width:100%;min-height:var(--tap);background:none;border:0;color:var(--text);text-align:left;cursor:pointer;font:400 13px var(--font-ui);padding:0}
  .to{color:var(--text-3);font-size:12px;flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  time{color:var(--text-3);font-size:12px}
  .bubble{background:var(--surface);border:1px solid var(--border);padding:12px 16px}
  .bubble.mine{background:var(--accent);border-color:var(--accent);color:var(--on-accent)}
  .text{white-space:pre-wrap;font-size:14px}
  .att{display:flex;align-items:center;gap:8px;margin-top:10px;flex-wrap:wrap}
  .chip{min-height:var(--tap);background:var(--fill);border:1px solid var(--border);color:var(--text);padding:0 12px;font:400 13px var(--font-ui);cursor:pointer}
  .link{min-height:var(--tap);background:none;border:0;color:var(--accent-light);font:400 12px var(--font-ui);cursor:pointer;padding:0}
  .quote{white-space:pre-wrap;color:var(--text-3);font-size:12px;border-left:1px solid var(--border);padding-left:12px}
  footer{display:flex;align-items:center;gap:16px;justify-content:flex-end;margin-top:16px;border-top:1px solid var(--border);padding-top:16px}
  .pill{margin-right:auto;background:var(--fill);color:var(--text-2);font-size:12px;padding:6px 12px}
</style>

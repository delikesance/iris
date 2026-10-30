<!--
  MailReader : lecture d'un message.
  Props : message (Full | null) · onback (retour à la liste, visible sous 700 px) · ondelete (supprimer le message).
  Actions : expéditeur → route contacts · « Modifier » → route settings/rules/<id> · crayon → signingTarget
  · télécharger → dialogue d'enregistrement + save_attachment · réponses rapides / Répondre → composeDraft.
-->
<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { saveAttachment } from "../../lib/api";
  import { composeDraft, route, selectedContact, signingTarget, toast } from "../../lib/stores";
  import type { Attachment, Full } from "../../lib/types";
  import SigningPanel from "../../components/signing/SigningPanel.svelte";
  import { threadRoute } from "../../lib/router";
  import { Avatar, Button, Chip, SectionLabel } from "../../components/ui";
  import { QUICK_REPLIES, fileExt, formatDateTime, formatSize, messageSrcdoc, replyDraft } from "./helpers";

  let { message, onback, ondelete }: { message: Full | null; onback: () => void; ondelete: () => void } = $props();

  async function download(m: Full, a: Attachment) {
    try {
      const dest = await save({ defaultPath: a.filename });
      if (dest) await saveAttachment(m.folder, m.uid, a.index, dest);
    } catch (e) {
      toast.set({ kind: "error", text: `Téléchargement impossible : ${e}` });
    }
  }

  const openContact = (m: Full) => {
    selectedContact.set({ id: null, name: m.from_name || m.from_addr, email: m.from_addr, notes: null });
    route.set("contacts");
  };

  const sign = (m: Full, a: Attachment) => signingTarget.set({ folder: m.folder, uid: m.uid, index: a.index });
</script>

<article aria-label="Lecture">
  {#if message}
    {@const m = message}
    <div class="top">
      <button type="button" class="back" onclick={onback}>← Retour</button>
      <button type="button" class="tool" onclick={ondelete}>Supprimer</button>
    </div>
    <h1>{m.subject || "(sans objet)"}</h1>
    <div class="meta">
      <Avatar name={m.from_name || m.from_addr} variant="blue" size={36} />
      <div class="who">
        <button type="button" class="sender" onclick={() => openContact(m)}>{m.from_name || m.from_addr}</button>
        <span class="to">À {m.to}</span>
      </div>
      <time>{formatDateTime(m.date)}</time>
    </div>

    {#if m.rule_id !== null && m.sorted_into}
      {@const ruleId = m.rule_id}
      <p class="note">
        <span>Classé dans {m.sorted_into} par la règle « {m.rule_name} »</span>
        <button type="button" class="link" onclick={() => route.set(`settings/rules/${ruleId}`)}>Modifier</button>
      </p>
    {/if}

    {#if m.thread_id}
      <button type="button" class="link thread" onclick={() => route.set(threadRoute(m.thread_id!))}>Voir le fil</button>
    {/if}

    <iframe title="Contenu du message" sandbox="" srcdoc={messageSrcdoc(m)}></iframe>

    {#if m.attachments.length > 0}
      <div class="attachments">
        <SectionLabel>Pièces jointes · {m.attachments.length}</SectionLabel>
        {#each m.attachments as a (a.index)}
          <div class="att">
            <Chip>{fileExt(a.filename)}</Chip>
            <span class="file"><span class="fname">{a.filename}</span><span class="size">{formatSize(a.size)}</span></span>
            <button type="button" class="icon" aria-label="Télécharger {a.filename}" onclick={() => download(m, a)}>↓</button>
            {#if a.signable}
              <button type="button" class="icon" aria-label="Signer {a.filename}" onclick={() => sign(m, a)}>✎</button>
            {/if}
          </div>
        {/each}
      </div>
    {/if}

    {#if $signingTarget && $signingTarget.folder === m.folder && $signingTarget.uid === m.uid}
      {@const t = $signingTarget}
      {#key `${t.folder}/${t.uid}/${t.index}`}
        <SigningPanel
          target={t} filename={m.attachments.find((a) => a.index === t.index)?.filename ?? ""}
          replyTo={m.from_addr} subject={m.subject} inReplyTo={m.message_id} onclose={() => signingTarget.set(null)}
        />
      {/key}
    {/if}

    <div class="reply">
      {#each QUICK_REPLIES as q (q.label)}
        <button type="button" class="quick" onclick={() => composeDraft.set(replyDraft(m, q.body))}>{q.label}</button>
      {/each}
      <Button onclick={() => composeDraft.set(replyDraft(m))}>Répondre</Button>
    </div>
  {:else}
    <p class="placeholder">Sélectionnez un mail pour le lire.</p>
  {/if}
</article>

<style>
  article{display:flex;flex-direction:column;gap:16px;min-height:0;overflow-y:auto;padding:16px 32px 24px;background:var(--bg)}
  .top{display:flex;justify-content:space-between}
  .back{display:none}
  .tool,.back,.link,.sender,.quick,.icon{background:transparent;border:0;cursor:pointer;font:400 14px var(--font-ui);color:var(--text-2)}
  .tool,.back{min-height:var(--tap);padding:0 8px}
  .tool:hover,.back:hover,.quick:hover,.icon:hover{background:var(--fill)}
  h1{margin:0;font:300 40px/1.1 var(--font-title);color:var(--text)}
  .meta{display:flex;align-items:center;gap:12px}
  .who{display:flex;flex-direction:column;flex:1;min-width:0}
  .sender{align-self:flex-start;padding:0;min-height:24px;color:var(--text);font-weight:500;text-align:left}
  .sender:hover,.link:hover{text-decoration:underline}
  .to,time{font-size:13px;color:var(--text-3)}
  .note{display:flex;justify-content:space-between;align-items:center;gap:12px;margin:0;padding:0 12px;min-height:var(--tap);background:var(--surface);border:1px solid var(--border);font-size:13px;color:var(--text-2)}
  .thread{align-self:flex-start}
  .link{color:var(--accent-light);padding:0 8px;min-height:var(--tap)}
  iframe{width:100%;min-height:260px;flex:1;border:1px solid var(--border);background:var(--surface)}
  .attachments{display:flex;flex-direction:column;gap:8px;padding:12px;background:var(--surface);border:1px solid var(--border)}
  .att{display:flex;align-items:center;gap:12px}
  .file{display:flex;flex-direction:column;flex:1;min-width:0}
  .fname{font-size:14px;color:var(--text);overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .size{font-size:12px;color:var(--text-3)}
  .icon{width:var(--tap);height:var(--tap);font-size:16px;border:1px solid var(--border)}
  .reply{display:flex;flex-wrap:wrap;gap:8px;align-items:center}
  .quick{min-height:var(--tap);padding:0 14px;border:1px solid var(--border)}
  .reply :global(button:last-child){margin-left:auto}
  button:focus-visible{outline:2px solid var(--accent-light);outline-offset:2px}
  .placeholder{margin:auto;color:var(--text-3)}
  @media (max-width:700px){
    article{padding:12px 16px}
    .back{display:inline-block}
    h1{font-size:32px}
  }
</style>

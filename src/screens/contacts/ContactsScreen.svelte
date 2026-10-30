<script lang="ts">
  import { onMount } from "svelte";
  import { Avatar, Button, Field, ScreenTitle, SectionLabel } from "../../components/ui";
  import { contactGet, contactSave, contactsList, ruleSentence, threadGet } from "../../lib/api";
  import { composeDraft, currentFolder, mailQuery, route, selectedContact, selectedUid, toast } from "../../lib/stores";
  import type { Contact, ContactDetail } from "../../lib/types";
  import { draftTo, formatDay, validContact } from "./contacts";

  let contacts = $state<Contact[]>([]);
  let query = $state("");
  let detail = $state<ContactDetail | null>(null);
  let sentence = $state("");
  let creating = $state(false);
  let newName = $state("");
  let newEmail = $state("");

  const fail = (e: unknown) => toast.set({ kind: "error", text: String(e) });

  async function search() {
    try { contacts = await contactsList(query.trim() || undefined); } catch (e) { fail(e); }
  }

  async function open(c: Contact) {
    creating = false;
    try {
      detail = await contactGet(c.email);
      const r = detail.rule;
      sentence = r ? await ruleSentence(r.cond, r.action) : "";
    } catch (e) { fail(e); }
  }

  async function create() {
    if (!validContact(newName, newEmail)) return;
    try {
      const saved = await contactSave({ id: null, name: newName.trim(), email: newEmail.trim(), notes: null });
      newName = ""; newEmail = "";
      await search();
      await open(saved);
    } catch (e) { fail(e); }
  }

  function showMessage(folder: string, uid: number) {
    currentFolder.set(folder);
    selectedUid.set(uid);
    route.set("mail");
  }

  async function openThread(threadId: string) {
    try {
      const msgs = await threadGet(threadId);
      const last = msgs[msgs.length - 1];
      if (last) showMessage(last.folder, last.uid);
    } catch (e) { fail(e); }
  }

  function allMails(email: string) {
    mailQuery.set(email);
    route.set("mail");
  }

  onMount(async () => {
    await search();
    const c = $selectedContact;
    if (!c) return;
    selectedContact.set(null);
    await open(c);
  });
</script>

<section class="screen">
  <aside>
    <div class="head">
      <ScreenTitle size={28}>Contacts</ScreenTitle>
      <Button onclick={() => { creating = true; detail = null; }}>Nouveau</Button>
    </div>
    <input type="search" placeholder="Rechercher un contact" aria-label="Rechercher un contact" bind:value={query} oninput={search} />
    <ul>
      {#each contacts as c (c.email)}
        <li><button type="button" class:on={detail?.contact.email === c.email} onclick={() => open(c)}>
          <Avatar name={c.name || c.email} variant="blue" />
          <span><strong>{c.name}</strong><small>{c.notes ?? c.email}</small></span>
        </button></li>
      {/each}
    </ul>
  </aside>

  <div class="fiche">
    {#if creating}
      <form onsubmit={(e) => { e.preventDefault(); create(); }}>
        <ScreenTitle size={32}>Nouveau contact</ScreenTitle>
        <Field label="Nom" bind:value={newName} />
        <Field label="Adresse e-mail" type="email" bind:value={newEmail} />
        <div class="actions">
          <Button type="submit" disabled={!validContact(newName, newEmail)}>Enregistrer</Button>
          <Button variant="secondary" onclick={() => (creating = false)}>Annuler</Button>
        </div>
      </form>
    {:else if detail}
      <header>
        <Avatar name={detail.contact.name || detail.contact.email} variant="blue" size={56} />
        <div>
          <ScreenTitle size={32}>{detail.contact.name}</ScreenTitle>
          <p>{detail.contact.email}</p>
        </div>
      </header>
      <div class="actions">
        <Button onclick={() => composeDraft.set(draftTo(detail!.contact.email))}>Écrire</Button>
        <Button variant="secondary" onclick={() => allMails(detail!.contact.email)}>Voir tous les mails</Button>
      </div>

      <SectionLabel>Échanges récents</SectionLabel>
      <ul class="threads">
        {#each detail.recent_threads as t (t.thread_id)}
          <li><button type="button" onclick={() => openThread(t.thread_id)}>
            <strong>{t.subject}</strong><small>{formatDay(t.date)}</small>
          </button></li>
        {:else}
          <li class="empty">Aucun échange récent.</li>
        {/each}
      </ul>

      {#if detail.rule}
        <SectionLabel>Classement</SectionLabel>
        <div class="rule">
          <span>{sentence}</span>
          {#if detail.rule.id !== null}
            {@const ruleId = detail.rule.id}
            <Button variant="secondary" onclick={() => route.set(`settings/rules/${ruleId}`)}>Modifier</Button>
          {/if}
        </div>
      {/if}
    {:else}
      <p class="empty">Choisis un contact.</p>
    {/if}
  </div>
</section>

<style>
  .screen{display:grid;grid-template-columns:300px 1fr;height:100%}
  aside{display:flex;flex-direction:column;gap:12px;padding:24px 16px;background:var(--surface);border-right:1px solid var(--border);overflow:auto}
  .head{display:flex;align-items:center;justify-content:space-between;gap:8px}
  input[type=search]{min-height:var(--tap);background:var(--bg);color:var(--text);border:1px solid var(--border);padding:0 12px;font:400 14px var(--font-ui)}
  input[type=search]:focus{outline:2px solid var(--accent-light)}
  ul{list-style:none;margin:0;padding:0}
  aside li button{display:flex;align-items:center;gap:12px;width:100%;min-height:var(--tap);padding:6px 8px;background:none;border:0;color:var(--text);text-align:left;cursor:pointer}
  aside li button.on,aside li button:hover{background:var(--fill)}
  aside span,.threads button{display:flex;flex-direction:column;min-width:0}
  strong{font:500 14px var(--font-ui)}
  small{font:400 12px var(--font-ui);color:var(--text-3)}
  .fiche{display:flex;flex-direction:column;gap:20px;padding:40px 56px;overflow:auto;align-items:flex-start}
  header{display:flex;align-items:center;gap:16px}
  header p{margin:4px 0 0;color:var(--text-2);font:400 14px var(--font-ui)}
  .actions{display:flex;gap:8px}
  form{display:flex;flex-direction:column;gap:16px;width:360px}
  .threads{width:100%}
  .threads button{flex-direction:row;justify-content:space-between;align-items:center;width:100%;min-height:var(--tap);background:none;border:0;border-bottom:1px solid var(--border);color:var(--text);cursor:pointer;text-align:left}
  .rule{display:flex;align-items:center;justify-content:space-between;gap:16px;width:100%;padding:8px 12px;background:var(--surface);color:var(--text-2);font:400 13px var(--font-ui)}
  .empty{color:var(--text-3);font:400 14px var(--font-ui);margin:0}
</style>

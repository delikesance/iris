<!--
  ComposerDialog : contenu du composeur. Props : draft (Draft initial) · onclose (fermeture).
  Envoi : api.sendMail avec send_at (créneau « Envoyer plus tard ») et remind_days (Relance).
-->
<script lang="ts">
  import { untrack } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import * as api from "../../lib/api";
  import { toast } from "../../lib/stores";
  import type { Draft } from "../../lib/types";
  import { Button, Chip, Toggle, SectionLabel } from "../../components/ui";
  import {
    quickSlots, customSlot, parseLocalInput, toUnix, mainButtonLabel, sendNowLabel,
    fileName, isSignedAttachment, reminderNote, type Slot,
  } from "./slots";

  let { draft, onclose }: { draft: Draft; onclose: () => void } = $props();

  const init = untrack(() => draft);
  let to = $state(init.to);
  let cc = $state(init.cc ?? "");
  let showCc = $state(!!init.cc);
  let subject = $state(init.subject);
  let body = $state(init.body);
  let attachments = $state<string[]>([...init.attachments]);
  let remind = $state(init.remind_days != null);
  let remindDays = $state(init.remind_days ?? 3);
  let slot = $state<Slot | null>(null);
  let panel = $state(false);
  let pickDate = $state(false);
  let dateValue = $state("");
  let busy = $state(false);

  const now = () => new Date();
  const slots = $derived(panel ? quickSlots(now()) : []);
  const canSend = $derived(to.trim().length > 0 && !busy);

  async function attach() {
    const picked = await open({ multiple: true });
    if (!picked) return;
    const paths = Array.isArray(picked) ? picked : [picked];
    attachments = [...new Set([...attachments, ...paths])];
  }

  function choose(s: Slot) { slot = s; pickDate = false; }

  function chooseDate() {
    const s = customSlot(parseLocalInput(dateValue), now());
    if (s) slot = s;
    else toast.set({ kind: "error", text: "Choisissez une date à venir." });
  }

  function build(sendAt: Slot | null): Draft {
    return {
      to: to.trim(), cc: cc.trim() || null, subject, body, in_reply_to: init.in_reply_to ?? null,
      attachments, remind_days: remind ? remindDays : null, send_at: sendAt ? toUnix(sendAt.at) : null,
    };
  }

  async function send(sendAt: Slot | null) {
    busy = true;
    try {
      const r = await api.sendMail(build(sendAt));
      toast.set({ kind: "info", text: r.status === "scheduled" && sendAt ? `Envoi planifié pour ${sendAt.label}.` : "Message envoyé." });
      onclose();
    } catch (e) {
      toast.set({ kind: "error", text: String(e) });
    } finally {
      busy = false;
    }
  }

  const keydown = (e: KeyboardEvent) => { if (e.key === "Escape") onclose(); };
</script>

<svelte:window onkeydown={keydown} />

<div class="backdrop" role="presentation">
  <div class="wrap">
    <div class="dialog" role="dialog" aria-modal="true" aria-label="Nouveau message">
      <header>
        <h2>Nouveau message</h2>
        <button type="button" class="icon" aria-label="Fermer" onclick={onclose}>×</button>
      </header>
      <div class="row">
        <label for="c-to">À</label>
        <input id="c-to" bind:value={to} autocomplete="off" />
        {#if !showCc}<button type="button" class="link" onclick={() => (showCc = true)}>Cc</button>{/if}
      </div>
      {#if showCc}
        <div class="row"><label for="c-cc">Cc</label><input id="c-cc" bind:value={cc} autocomplete="off" /></div>
      {/if}
      <div class="row"><label for="c-subject">Objet</label><input id="c-subject" bind:value={subject} /></div>
      <textarea aria-label="Message" bind:value={body}></textarea>

      {#if attachments.length}
        <ul class="files">
          {#each attachments as path (path)}
            <li>
              <span class="ext">{(fileName(path).split(".").pop() ?? "").toUpperCase().slice(0, 4)}</span>
              <span class="meta">
                <span class="name">{fileName(path)}</span>
                {#if isSignedAttachment(path)}<span class="signed">✓ signé par vous, avec journal de preuve</span>{/if}
              </span>
              <button type="button" class="icon" aria-label="Retirer {fileName(path)}" onclick={() => (attachments = attachments.filter(p => p !== path))}>×</button>
            </li>
          {/each}
        </ul>
      {/if}

      <footer>
        <button type="button" class="icon" aria-label="Joindre un fichier" onclick={attach}>+</button>
        <span class="spacer"></span>
        {#if slot}
          <Button variant="secondary" disabled={!canSend} onclick={() => send(null)}>{sendNowLabel}</Button>
        {:else}
          <Button variant="secondary" onclick={() => (panel = !panel)}>Envoyer plus tard</Button>
        {/if}
        <Button disabled={!canSend} onclick={() => send(slot)}>{mainButtonLabel(slot)}</Button>
      </footer>
    </div>

    {#if panel}
      <aside aria-label="Envoyer plus tard">
        <SectionLabel>Envoyer plus tard</SectionLabel>
        <ul>
          {#each slots as s (s.id)}
            <li><button type="button" class:active={slot?.id === s.id} aria-pressed={slot?.id === s.id} onclick={() => choose(s)}><span>{s.title}</span><span>{s.time}{slot?.id === s.id ? " ✓" : ""}</span></button></li>
          {/each}
          <li><button type="button" class:active={slot?.id === "custom"} onclick={() => (pickDate = !pickDate)}><span>Choisir une date…</span>{#if slot?.id === "custom"}<span>{slot.time} ✓</span>{/if}</button></li>
        </ul>
        {#if pickDate}
          <div class="date">
            <input type="datetime-local" aria-label="Date d'envoi" bind:value={dateValue} onchange={chooseDate} />
          </div>
        {/if}
        {#if slot}<button type="button" class="link" onclick={() => (slot = null)}>Annuler la planification</button>{/if}
        <div class="remind">
          <div class="line"><strong>Relancer sans réponse</strong><Toggle bind:checked={remind} label="Relancer sans réponse" /></div>
          <small>{reminderNote(to, remindDays)}</small>
          {#if remind}
            <label class="days">Délai en jours
              <input type="number" min="1" max="30" bind:value={remindDays} />
            </label>
          {/if}
        </div>
      </aside>
    {/if}
  </div>
</div>

<style>
  .backdrop{position:fixed;inset:0;background:rgba(13,13,32,.8);display:flex;align-items:center;justify-content:center;z-index:50}
  .wrap{display:flex;gap:12px;align-items:flex-start;max-width:96vw;max-height:94vh}
  .dialog{width:640px;max-width:100%;background:var(--surface);border:1px solid var(--border);display:flex;flex-direction:column}
  aside{width:260px;background:var(--surface);border:1px solid var(--border);padding:16px;display:flex;flex-direction:column;gap:12px}
  header{display:flex;align-items:center;justify-content:space-between;padding:0 8px 0 20px;border-bottom:1px solid var(--border)}
  h2{margin:0;font:500 15px var(--font-ui)}
  .row{display:flex;align-items:center;gap:12px;padding:0 20px;border-bottom:1px solid var(--border);min-height:var(--tap)}
  .row label{width:44px;color:var(--text-3);font-size:13px}
  .row input{flex:1;min-height:var(--tap);background:none;border:0;color:var(--text);font:400 15px var(--font-ui)}
  input:focus,textarea:focus{outline:2px solid var(--accent-light);outline-offset:-2px}
  textarea{min-height:220px;background:none;border:0;color:var(--text);padding:16px 20px;font:400 15px/1.6 var(--font-ui);resize:vertical}
  .icon{min-width:var(--tap);min-height:var(--tap);background:none;border:0;color:var(--text-2);font-size:20px;cursor:pointer}
  .link{min-height:var(--tap);background:none;border:0;color:var(--accent-light);font:400 13px var(--font-ui);cursor:pointer;padding:0 8px}
  .files{list-style:none;margin:0 20px 12px;padding:0}
  .files li{display:flex;align-items:center;gap:12px;background:var(--bg);border:1px solid var(--border);padding:4px 4px 4px 12px}
  .ext{background:var(--fill);color:var(--text-2);font-size:11px;padding:2px 6px}
  .meta{flex:1;display:flex;flex-direction:column;font-size:13px}
  .signed{color:var(--accent-light);font-size:12px}
  footer{display:flex;align-items:center;gap:12px;padding:8px 12px;border-top:1px solid var(--border)}
  .spacer{flex:1}
  aside ul{list-style:none;margin:0;padding:0}
  aside li button{width:100%;min-height:var(--tap);display:flex;justify-content:space-between;align-items:center;background:none;border:0;color:var(--text);font:400 14px var(--font-ui);padding:0 12px;cursor:pointer}
  aside li button:hover{background:var(--fill)}
  aside li button.active{background:var(--accent);color:var(--on-accent)}
  .date input,.days input{min-height:var(--tap);background:var(--bg);border:1px solid var(--border);color:var(--text);padding:0 12px;font:400 14px var(--font-ui);width:100%}
  .remind{border-top:1px solid var(--border);padding-top:12px;display:flex;flex-direction:column;gap:4px}
  .line{display:flex;align-items:center;justify-content:space-between;font-size:13px}
  small{color:var(--text-3);font-size:12px}
  .days{display:flex;flex-direction:column;gap:6px;font-size:12px;color:var(--text-3)}
</style>

<!--
  Login : image fixe à gauche, formulaire à droite (seule cette colonne défile).
  Props : initial (Settings | null, réglages mémorisés) · onsuccess (reçoit l'adresse connectée).
-->
<script lang="ts">
  import { Button, Field, ScreenTitle, SectionLabel } from "../../components/ui";
  import { login } from "../../lib/api";
  import type { Security, Settings } from "../../lib/types";
  import hero from "../../assets/login/login-hero.jpg";
  import {
    SECURITY_OPTIONS, defaultSettings, portAfterSecurityChange, serversSummary, validPort, type Proto,
  } from "./security";

  let { initial = null, onsuccess }: { initial?: Settings | null; onsuccess: (email: string) => void } = $props();

  const start = (() => initial ?? defaultSettings())();
  let email = $state(start.email);
  let password = $state("");
  let imapHost = $state(start.imap_host);
  let smtpHost = $state(start.smtp_host);
  let imapPort = $state(String(start.imap_port));
  let smtpPort = $state(String(start.smtp_port));
  let imapSec = $state<Security>(start.imap_security ?? "ssl");
  let smtpSec = $state<Security>(start.smtp_security ?? "ssl");
  let expanded = $state(false);
  let busy = $state(false);
  let error = $state("");

  const settings = $derived<Settings>({
    email: email.trim(), imap_host: imapHost.trim(), smtp_host: smtpHost.trim(),
    imap_port: Number(imapPort), smtp_port: Number(smtpPort), imap_security: imapSec, smtp_security: smtpSec,
  });
  const canSubmit = $derived(
    !busy && settings.email !== "" && password !== "" && settings.imap_host !== "" && settings.smtp_host !== ""
    && validPort(settings.imap_port) && validPort(settings.smtp_port),
  );

  function changeSecurity(proto: Proto, to: Security) {
    if (proto === "imap") { imapPort = String(portAfterSecurityChange("imap", imapSec, to, Number(imapPort))); imapSec = to; }
    else { smtpPort = String(portAfterSecurityChange("smtp", smtpSec, to, Number(smtpPort))); smtpSec = to; }
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!canSubmit) return;
    busy = true; error = "";
    try { await login(settings, password); onsuccess(settings.email); }
    catch (err) { error = String(err); }
    finally { busy = false; }
  }
</script>

<main>
  <img src={hero} alt="Iris — Le courrier qui se trie tout seul." />
  <section>
    <form onsubmit={submit}>
      <ScreenTitle>Connexion</ScreenTitle>
      <p class="lead">Entrez votre adresse et votre mot de passe. Les serveurs par défaut sont déjà configurés.</p>

      <SectionLabel>Compte</SectionLabel>
      <Field label="Adresse mail" name="email" type="email" bind:value={email} placeholder="vous@exemple.fr" />
      <Field label="Mot de passe" name="password" type="password" bind:value={password} />

      <div class="servers">
        <div class="head">
          <div>
            <SectionLabel>Serveurs · par défaut</SectionLabel>
            <div class="summary">{serversSummary(settings)}</div>
          </div>
          <button type="button" class="edit" aria-expanded={expanded} onclick={() => (expanded = !expanded)}>
            Modifier <span aria-hidden="true">{expanded ? "⌃" : "⌄"}</span>
          </button>
        </div>

        {#if expanded}
          {#each [["imap", "Réception (IMAP)"], ["smtp", "Envoi (SMTP)"]] as const as [proto, title] (proto)}
            <fieldset>
              <legend>{title}</legend>
              {#if proto === "imap"}
                <Field label="Serveur" name="imap_host" bind:value={imapHost} />
              {:else}
                <Field label="Serveur" name="smtp_host" bind:value={smtpHost} />
              {/if}
              <div class="row">
                {#if proto === "imap"}
                  <Field label="Port" name="imap_port" type="number" bind:value={imapPort} />
                {:else}
                  <Field label="Port" name="smtp_port" type="number" bind:value={smtpPort} />
                {/if}
                <label class="sec">
                  <span>Sécurité</span>
                  <select name={`${proto}_security`} value={proto === "imap" ? imapSec : smtpSec}
                    onchange={(e) => changeSecurity(proto, e.currentTarget.value as Security)}>
                    {#each SECURITY_OPTIONS as o (o.id)}<option value={o.id}>{o.label}</option>{/each}
                  </select>
                </label>
              </div>
            </fieldset>
          {/each}
        {/if}
      </div>

      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <Button type="submit" disabled={!canSubmit}>{busy ? "Connexion…" : "Se connecter"}</Button>
    </form>
  </section>
</main>

<style>
  main{display:flex;height:100vh;overflow:hidden;background:var(--bg)}
  img{flex:none;width:min(52vw,740px);height:100%;object-fit:cover;object-position:left center}
  section{flex:1;min-width:0;overflow-y:auto}
  form{width:min(440px,100% - 48px);margin:0 auto;min-height:100%;padding:64px 0;display:flex;flex-direction:column;justify-content:center;gap:16px}
  .lead{margin:0;color:var(--text-2);font-size:14px}
  .servers{background:var(--surface);border:1px solid var(--border);padding:12px 16px;display:flex;flex-direction:column;gap:12px}
  .head{display:flex;align-items:center;justify-content:space-between;gap:12px}
  .summary{font-size:13px;color:var(--text)}
  .edit{min-height:var(--tap);background:none;border:0;color:var(--accent-light);font:500 13px var(--font-ui);cursor:pointer}
  fieldset{border:0;margin:0;padding:0;display:flex;flex-direction:column;gap:12px}
  legend{padding:0 0 8px;font:500 11px var(--font-ui);letter-spacing:.12em;text-transform:uppercase;color:var(--text-3)}
  .row{display:grid;grid-template-columns:1fr 1fr;gap:12px}
  .sec{display:flex;flex-direction:column;gap:6px}
  .sec span{font:400 13px var(--font-ui);color:var(--text-3)}
  select{min-height:var(--tap);background:var(--surface);color:var(--text);border:1px solid var(--border);padding:0 12px;font:400 15px var(--font-ui)}
  select:focus-visible,.edit:focus-visible{outline:2px solid var(--accent-light)}
  .error{margin:0;color:var(--text);border-left:3px solid var(--accent-light);padding:8px 12px;background:var(--fill);font-size:14px}
  @media (max-width:700px){img{display:none}}
</style>

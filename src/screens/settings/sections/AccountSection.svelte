<!-- AccountSection : boîte mail connectée, serveurs, déconnexion. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { Button, SectionLabel } from "../../../components/ui";
  import { account, logout, savedSettings } from "../../../lib/api";
  import { route, toast } from "../../../lib/stores";
  import type { Settings } from "../../../lib/types";

  let email = $state("");
  let settings = $state<Settings | null>(null);

  onMount(async () => {
    try {
      [email, settings] = await Promise.all([account(), savedSettings()]);
    } catch (e) {
      toast.set({ kind: "error", text: String(e) });
    }
  });

  async function leave() {
    try {
      await logout();
      route.set("login");
    } catch (e) {
      toast.set({ kind: "error", text: String(e) });
    }
  }
</script>

<SectionLabel>Boîte mail</SectionLabel>
<dl>
  <dt>Adresse</dt><dd>{email}</dd>
  {#if settings}
    <dt>Réception IMAP</dt><dd>{settings.imap_host}:{settings.imap_port}</dd>
    <dt>Envoi SMTP</dt><dd>{settings.smtp_host}:{settings.smtp_port}</dd>
  {/if}
</dl>
<Button variant="secondary" onclick={leave}>Se déconnecter</Button>

<style>
  dl{display:grid;grid-template-columns:max-content 1fr;gap:8px 24px;margin:16px 0 24px}
  dt{color:var(--text-3);font-size:14px}
  dd{margin:0}
</style>

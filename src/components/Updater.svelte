<script lang="ts">
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";

  let pending = $state<Update | null>(null);
  let text = $state("");
  let busy = $state(false);

  async function checkUpdate() {
    try {
      const u = await check();
      if (!u) return;
      pending = u;
      text = `Mettre à jour (v${u.version})`;
    } catch (e) { console.warn("update check:", e); }
  }

  async function install() {
    if (!pending) return;
    busy = true;
    let total = 0, done = 0;
    try {
      await pending.downloadAndInstall(ev => {
        if (ev.event === "Started") total = ev.data.contentLength ?? 0;
        else if (ev.event === "Progress") { done += ev.data.chunkLength; text = total ? `Téléchargement ${Math.round(done / total * 100)} %` : "Téléchargement…"; }
        else if (ev.event === "Finished") text = "Installation…";
      });
      await relaunch();
    } catch (e) { busy = false; text = "Échec — réessayer"; console.error(e); }
  }

  // Vérification au démarrage puis toutes les 6 h
  $effect(() => {
    const t = setTimeout(checkUpdate, 3000);
    const i = setInterval(checkUpdate, 6 * 3600 * 1000);
    return () => { clearTimeout(t); clearInterval(i); };
  });
</script>

{#if pending}
  <button class="update" disabled={busy} onclick={install}>{text}</button>
{/if}

<style>
  .update{min-height:var(--tap);width:100%;padding:0 12px;background:var(--accent);color:var(--on-accent);border:0;font:500 13px var(--font-ui);cursor:pointer}
  .update:hover:not(:disabled){background:var(--accent-alt)}
  .update:focus-visible{outline:2px solid var(--accent-light);outline-offset:2px}
  .update:disabled{opacity:.5;cursor:default}
</style>

<script lang="ts">
  import "./styles/index.css";
  import { onMount } from "svelte";
  import type { Component } from "svelte";
  import { account, logout, savedSettings } from "./lib/api";
  import { composeDraft, route, toast } from "./lib/stores";
  import { screenOf, threadIdOf, type Screen } from "./lib/router";
  import type { Settings } from "./lib/types";
  import Sidebar from "./components/Sidebar.svelte";
  import Composer from "./screens/compose/Composer.svelte";
  import ThreadView from "./screens/thread/ThreadView.svelte";
  import Login from "./screens/login/Login.svelte";

  // Un fichier par écran ; chargés à la demande.
  const loaders = import.meta.glob<{ default: Component }>("./screens/*/*Screen.svelte");
  const PATHS: Record<Exclude<Screen, "login" | "thread">, string> = {
    mail: "mail/MailScreen", contacts: "contacts/ContactsScreen", recurring: "recurring/RecurringScreen",
    chrons: "chrons/ChronsScreen", signatures: "signatures/SignaturesScreen", settings: "settings/SettingsScreen",
  };

  let ready = $state(false);
  let email = $state("");
  let saved = $state<Settings | null>(null);

  const screen = $derived(screenOf($route));
  const threadId = $derived(threadIdOf($route));
  const Current = $derived(screen === "login" ? null : screen === "thread" ? undefined : loaders[`./screens/${PATHS[screen]}.svelte`]().then(m => m.default));

  onMount(() => {
    document.body.classList.add("grain");
    (async () => {
      try { email = await account(); if ($route === "login") route.set("mail"); }
      catch { saved = await savedSettings().catch(() => null); route.set("login"); }
      ready = true;
    })();
    return () => document.body.classList.remove("grain");
  });

  function loggedIn(addr: string) { email = addr; route.set("mail"); }

  async function signOut() {
    await logout().catch(() => {});
    email = ""; composeDraft.set(null); route.set("login");
  }

  $effect(() => {
    if (!$toast) return;
    const t = setTimeout(() => toast.set(null), 4000);
    return () => clearTimeout(t);
  });
</script>

{#if ready}
  {#if Current === null}
    <Login initial={saved} onsuccess={loggedIn} />
  {:else}
    <div class="shell">
      <Sidebar {email} onlogout={signOut} />
      <main>
        {#if threadId !== null}
          <ThreadView {threadId} onclose={() => route.set("mail")} />
        {:else if Current}
          {#await Current then Screen}<Screen />{/await}
        {/if}
      </main>
    </div>
    {#if $composeDraft}<Composer />{/if}
  {/if}
{/if}

{#if $toast}
  <div class="toast" class:error={$toast.kind === "error"} role="status">{$toast.text}</div>
{/if}

<style>
  .shell{display:flex;height:100vh}
  main{flex:1;min-width:0;overflow:auto}
  .toast{position:fixed;left:50%;bottom:24px;transform:translateX(-50%);padding:12px 20px;background:var(--fill);color:var(--text);border:1px solid var(--border);border-left:3px solid var(--accent-light);font-size:14px}
  .toast.error{border-left-color:var(--text)}
</style>

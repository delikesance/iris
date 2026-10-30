<!--
  Sidebar : rail de navigation (fond --sidebar). Props : email (string, compte connecté) · onlogout (handler).
  Lit/écrit le store `route`. Sélecteur de boîte en bas à gauche, avec « Se déconnecter ».
-->
<script lang="ts">
  import { route } from "../lib/stores";
  import { NAV, isActive } from "../lib/router";
  import IrisMark from "./IrisMark.svelte";
  import Updater from "./Updater.svelte";
  import { Avatar } from "./ui";

  let { email, onlogout }: { email: string; onlogout: () => void } = $props();
  let open = $state(false);
</script>

<nav aria-label="Navigation principale">
  <div class="brand"><IrisMark size={28} color="var(--accent-light)" /><span>Iris</span></div>

  <ul>
    {#each NAV as entry (entry.route)}
      <li>
        <button type="button" class:on={isActive(entry, $route)} aria-current={isActive(entry, $route) ? "page" : undefined}
          onclick={() => route.set(entry.route)}>{entry.label}</button>
      </li>
    {/each}
  </ul>

  <div class="bottom">
    <Updater />
    {#if open}
      <button type="button" class="logout" onclick={onlogout}>Se déconnecter</button>
    {/if}
    <button type="button" class="mailbox" aria-expanded={open} title={email} onclick={() => (open = !open)}>
      <Avatar name={email} variant="me" size={28} />
      <span class="addr">{email}</span>
    </button>
  </div>
</nav>

<style>
  nav{width:220px;flex:none;display:flex;flex-direction:column;background:var(--sidebar);border-right:1px solid var(--border);padding:16px 0 0}
  .brand{display:flex;align-items:center;gap:10px;padding:0 20px 20px;font:300 26px var(--font-title);letter-spacing:.04em;color:var(--text)}
  ul{list-style:none;margin:0;padding:0;flex:1}
  li button{width:100%;min-height:var(--tap);display:flex;align-items:center;padding:0 20px;background:none;border:0;border-left:3px solid transparent;color:var(--text-2);font:500 15px var(--font-ui);text-align:left;cursor:pointer}
  li button:hover{background:var(--fill)}
  li button.on{color:var(--text);background:var(--fill);border-left-color:var(--accent-light)}
  .bottom{display:flex;flex-direction:column;border-top:1px solid var(--border)}
  .mailbox,.logout{min-height:var(--tap);display:flex;align-items:center;gap:10px;padding:0 20px;background:none;border:0;color:var(--text-2);font:400 13px var(--font-ui);text-align:left;cursor:pointer}
  .mailbox:hover,.logout:hover{background:var(--fill)}
  .logout{color:var(--text)}
  .addr{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  button:focus-visible{outline:2px solid var(--accent-light);outline-offset:-2px}
</style>

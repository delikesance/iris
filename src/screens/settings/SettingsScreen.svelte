<!--
  SettingsScreen : réglages en sections (registre sections.ts). Le lien profond settings/rules/<id>
  ouvre Automatisation sur la règle <id>.
-->
<script lang="ts">
  import { ScreenTitle } from "../../components/ui";
  import { route } from "../../lib/stores";
  import { findSection, sectionForRule, sections } from "./sections";
  import { parseRuleRoute } from "./ruleBuilder";

  let picked = $state<string | null>(null);
  const ruleId = $derived(parseRuleRoute($route));
  const active = $derived(findSection(ruleId != null ? sectionForRule : picked));
  const Active = $derived(active.component);

  function pick(id: string) {
    picked = id;
    if (ruleId != null) route.set("settings");
  }
</script>

<div class="screen">
  <nav aria-label="Sections des réglages">
    <ScreenTitle size={32}>Réglages</ScreenTitle>
    <ul>
      {#each sections as s (s.id)}
        <li>
          <button type="button" class:on={s.id === active.id} aria-current={s.id === active.id ? "page" : undefined} onclick={() => pick(s.id)}>{s.label}</button>
        </li>
      {/each}
    </ul>
  </nav>
  <main>
    <ScreenTitle>{active.label}</ScreenTitle>
    <div class="body"><Active {ruleId} /></div>
  </main>
</div>

<style>
  .screen{display:grid;grid-template-columns:240px 1fr;height:100%;min-height:0}
  nav{background:var(--sidebar);padding:24px 16px;display:flex;flex-direction:column;gap:16px;overflow:auto}
  ul{list-style:none;margin:0;padding:0;display:flex;flex-direction:column}
  li button{width:100%;min-height:var(--tap);text-align:left;padding:0 12px;background:none;border:0;border-left:2px solid transparent;color:var(--text-2);font:400 15px var(--font-ui);cursor:pointer}
  li button.on{color:var(--text);background:var(--fill);border-left-color:var(--accent-light)}
  li button:focus-visible{outline:2px solid var(--accent-light)}
  main{padding:32px 40px;overflow:auto}
  .body{margin-top:24px;max-width:860px}
  @media (max-width:700px){.screen{grid-template-columns:1fr}main{padding:20px}}
</style>

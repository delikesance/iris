<!-- AutomationSection : Règles de tri (liste + constructeur) et accès aux Chrons. Props : ruleId (lien profond). -->
<script lang="ts">
  import { Button, SectionLabel } from "../../../components/ui";
  import { route } from "../../../lib/stores";
  import RulesSection from "../RulesSection.svelte";

  let { ruleId = null }: { ruleId?: number | null } = $props();
</script>

<SectionLabel>Règles de tri</SectionLabel>
<div class="gap"></div>
<RulesSection editId={ruleId} />

<SectionLabel>Chrons</SectionLabel>
<p>Des tâches planifiées, avec l'expression cron visible.</p>
<Button variant="secondary" onclick={() => route.set("chrons")}>Ouvrir les chrons</Button>

<style>
  .gap{height:12px}
  p{color:var(--text-2);font-size:14px}
</style>

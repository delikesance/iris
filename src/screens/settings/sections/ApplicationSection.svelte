<!-- ApplicationSection : fonctionnement de l'application (information). -->
<script lang="ts">
  import { SectionLabel } from "../../../components/ui";
</script>

<SectionLabel>Planification</SectionLabel>
<p>Les chrons, les envois planifiés et les relances s'exécutent dans l'application : ils ne tournent que lorsqu'Iris est ouvert.
  Au démarrage, une tâche manquée depuis moins de 24 heures est rattrapée une fois.</p>
<SectionLabel>Notifications</SectionLabel>
<p>Les notifications du système s'affichent pour les règles dont l'action est « notifier ».</p>

<style>
  p{color:var(--text-2);max-width:60ch;margin:12px 0 24px}
</style>

<!--
  Composer : fenêtre « Nouveau message » (PDF p.12). Ouverte quand le store composeDraft n'est pas null ;
  se ferme en le remettant à null. Aucune prop. Le contenu vit dans ComposerDialog (clé = brouillon).
-->
<script lang="ts">
  import { composeDraft } from "../../lib/stores";
  import ComposerDialog from "./ComposerDialog.svelte";
</script>

{#if $composeDraft}
  {#key $composeDraft}
    <ComposerDialog draft={$composeDraft} onclose={() => composeDraft.set(null)} />
  {/key}
{/if}

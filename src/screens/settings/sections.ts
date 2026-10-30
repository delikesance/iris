import type { Component } from "svelte";
import AccountSection from "./sections/AccountSection.svelte";
import ApplicationSection from "./sections/ApplicationSection.svelte";
import AutomationSection from "./sections/AutomationSection.svelte";
import DataSection from "./sections/DataSection.svelte";
import ShortcutsSection from "./sections/ShortcutsSection.svelte";
import SignatureSection from "./sections/SignatureSection.svelte";

export type Section = {
  id: string;
  label: string;
  /** `ruleId` : règle ouverte par le lien profond settings/rules/<id> (Automatisation seulement). */
  component: Component<any>;
};

/** Registre des sections : en ajouter une = ajouter une ligne, la navigation ne change pas. */
export const sections: Section[] = [
  { id: "compte", label: "Compte", component: AccountSection },
  { id: "automatisation", label: "Automatisation", component: AutomationSection },
  { id: "application", label: "Application", component: ApplicationSection },
  { id: "donnees", label: "Données", component: DataSection },
  { id: "signatures", label: "Signatures", component: SignatureSection },
  { id: "raccourcis", label: "Raccourcis clavier", component: ShortcutsSection },
];

export const sectionForRule = "automatisation";
export const defaultSection = sections[0].id;

export function findSection(id: string | null): Section {
  return sections.find((s) => s.id === id) ?? sections[0];
}

import { beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { get } from "svelte/store";
import type { Rule } from "../../lib/types";

const api = vi.hoisted(() => ({
  rulesList: vi.fn(), ruleSave: vi.fn(), ruleDelete: vi.fn(), ruleToggle: vi.fn(), ruleSentence: vi.fn(),
  account: vi.fn(), savedSettings: vi.fn(), logout: vi.fn(), signatureImageGet: vi.fn(), signatureImageSave: vi.fn(),
}));
vi.mock("../../lib/api", () => api);

import { route } from "../../lib/stores";
import SettingsScreen from "./SettingsScreen.svelte";
import { sections } from "./sections";

const rule = (id: number, over: Partial<Rule> = {}): Rule => ({
  id, name: `r${id}`, enabled: true, position: id,
  cond: { type: "sender_is", value: "atelier-norge.fr" }, action: { type: "move_to", folder: "Clients" }, ...over,
});

beforeEach(() => {
  cleanup();
  vi.clearAllMocks();
  route.set("settings");
  api.rulesList.mockResolvedValue([rule(1), rule(2, { enabled: false, cond: { type: "has_list_unsubscribe" }, action: { type: "move_to", folder: "Newsletters" } })]);
  api.ruleSentence.mockImplementation(async (c: { type: string; value?: string }, a: { type: string; folder?: string }) =>
    `Si ${c.type} ${c.value ?? ""}, alors ${a.type} ${a.folder ?? ""}`.replace(/\s+/g, " "));
  api.account.mockResolvedValue("moi@fastmail.com");
  api.savedSettings.mockResolvedValue(null);
  api.signatureImageGet.mockResolvedValue(null);
});

describe("registre des sections", () => {
  it("contient les 6 sections demandées, ids uniques", () => {
    expect(sections.map((s) => s.label)).toEqual(["Compte", "Automatisation", "Application", "Données", "Signatures", "Raccourcis clavier"]);
    expect(new Set(sections.map((s) => s.id)).size).toBe(sections.length);
  });
});

describe("SettingsScreen", () => {
  it("affiche Compte par défaut et change de section sans toucher à la route", async () => {
    render(SettingsScreen);
    expect(await screen.findByText("moi@fastmail.com")).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Raccourcis clavier" }));
    expect(await screen.findByText("Tout sélectionner dans la liste des messages")).toBeTruthy();
    expect(get(route)).toBe("settings");
  });

  it("Automatisation liste les règles en phrases avec interrupteur", async () => {
    render(SettingsScreen);
    await fireEvent.click(screen.getByRole("button", { name: "Automatisation" }));
    expect(await screen.findByText("Si sender_is atelier-norge.fr, alors move_to Clients")).toBeTruthy();
    expect(screen.getByText("Désactivée")).toBeTruthy();
    const sw = screen.getAllByRole("switch");
    expect(sw[0].getAttribute("aria-checked")).toBe("true");
    expect(sw[1].getAttribute("aria-checked")).toBe("false");
    await fireEvent.click(sw[1]);
    await waitFor(() => expect(api.ruleToggle).toHaveBeenCalledWith(2, true));
  });

  it("le lien profond settings/rules/2 ouvre Automatisation et le constructeur sur la règle 2", async () => {
    route.set("settings/rules/2");
    render(SettingsScreen);
    expect(await screen.findByRole("region", { name: "Modifier la règle" })).toBeTruthy();
    expect((screen.getByRole("button", { name: "Automatisation" })).getAttribute("aria-current")).toBe("page");
    await waitFor(() => expect(screen.getByTestId("rule-sentence").textContent).toContain("has_list_unsubscribe"));
  });

  it("lien profond vers une règle inconnue : message, pas de constructeur", async () => {
    route.set("settings/rules/99");
    render(SettingsScreen);
    expect(await screen.findByText("Cette règle n'existe plus.")).toBeTruthy();
    expect(screen.queryByRole("region", { name: "Modifier la règle" })).toBeNull();
  });

  it("Modifier met le lien profond dans la route", async () => {
    render(SettingsScreen);
    await fireEvent.click(screen.getByRole("button", { name: "Automatisation" }));
    await screen.findByText(/atelier-norge/);
    await fireEvent.click(screen.getAllByRole("button", { name: "Modifier" })[0]);
    expect(get(route)).toBe("settings/rules/1");
    expect(await screen.findByRole("region", { name: "Modifier la règle" })).toBeTruthy();
  });

  it("le constructeur met la phrase à jour en direct et enregistre", async () => {
    api.ruleSave.mockImplementation(async (r: Rule) => ({ ...r, id: 3 }));
    render(SettingsScreen);
    await fireEvent.click(screen.getByRole("button", { name: "Automatisation" }));
    await screen.findByText(/atelier-norge/);
    await fireEvent.click(screen.getByRole("button", { name: "Nouvelle règle" }));
    const save = await screen.findByRole("button", { name: "Enregistrer la règle" });
    expect((save as HTMLButtonElement).disabled).toBe(true);

    await fireEvent.input(screen.getByLabelText("Valeur de la condition"), { target: { value: "facture.fr" } });
    await fireEvent.input(screen.getByLabelText("Valeur de l'action"), { target: { value: "Factures" } });
    await waitFor(() => expect(screen.getByTestId("rule-sentence").textContent).toContain("Si sender_is facture.fr, alors move_to Factures"), { timeout: 2000 });
    expect(api.ruleSentence).toHaveBeenLastCalledWith({ type: "sender_is", value: "facture.fr" }, { type: "move_to", folder: "Factures" });

    await fireEvent.click(save);
    await waitFor(() => expect(api.ruleSave).toHaveBeenCalled());
    expect(api.ruleSave.mock.calls[0][0]).toMatchObject({
      id: null, enabled: true, position: 2, name: "Si sender_is facture.fr, alors move_to Factures",
      cond: { type: "sender_is", value: "facture.fr" }, action: { type: "move_to", folder: "Factures" },
    });
  });

  it("deux conditions sont envoyées en {type:'all'}", async () => {
    render(SettingsScreen);
    await fireEvent.click(screen.getByRole("button", { name: "Automatisation" }));
    await screen.findByText(/atelier-norge/);
    await fireEvent.click(screen.getByRole("button", { name: "Nouvelle règle" }));
    await fireEvent.input(await screen.findByLabelText("Valeur de la condition"), { target: { value: "a.fr" } });
    await fireEvent.click(screen.getByRole("button", { name: "+ Ajouter une condition" }));
    await fireEvent.input(screen.getAllByLabelText("Valeur de la condition")[1], { target: { value: "b.fr" } });
    await fireEvent.input(screen.getByLabelText("Valeur de l'action"), { target: { value: "X" } });
    await waitFor(() => expect(api.ruleSentence.mock.lastCall![0]).toEqual({
      type: "all", of: [{ type: "sender_is", value: "a.fr" }, { type: "sender_is", value: "b.fr" }],
    }), { timeout: 2000 });
  });

  it("Supprimer demande une confirmation", async () => {
    render(SettingsScreen);
    await fireEvent.click(screen.getByRole("button", { name: "Automatisation" }));
    await screen.findByText(/atelier-norge/);
    await fireEvent.click(screen.getAllByRole("button", { name: "Supprimer" })[0]);
    expect(api.ruleDelete).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "Confirmer la suppression" }));
    await waitFor(() => expect(api.ruleDelete).toHaveBeenCalledWith(1));
  });

  it("Se déconnecter appelle logout et renvoie à la connexion", async () => {
    api.logout.mockResolvedValue(undefined);
    render(SettingsScreen);
    await fireEvent.click(await screen.findByRole("button", { name: "Se déconnecter" }));
    await waitFor(() => expect(get(route)).toBe("login"));
  });
});

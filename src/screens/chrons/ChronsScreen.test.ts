import { beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import type { Chron, CronDescription } from "../../lib/types";

const api = vi.hoisted(() => ({
  chronsList: vi.fn(), chronSave: vi.fn(), chronToggle: vi.fn(), chronDelete: vi.fn(), cronDescribe: vi.fn(),
}));
vi.mock("../../lib/api", () => api);

import ChronsScreen from "./ChronsScreen.svelte";

const chron = (id: number, over: Partial<Chron> = {}): Chron => ({
  id, name: `Chron ${id}`, cron: "0 8 * * 1-5", kind: "morning_digest", enabled: true, last_run: null,
  next_run: Date.UTC(2026, 10, 2, 7, 0) / 1000, description: "Jours ouvrés, 08:00", ...over,
});
const ok = (d: string): CronDescription => ({ ok: true, description: d, next_runs: ["2026-11-02T07:00:00Z", "2026-11-03T07:00:00Z", "2026-11-04T07:00:00Z"], error: null });

beforeEach(() => {
  cleanup();
  vi.clearAllMocks();
  api.chronsList.mockResolvedValue([
    chron(1),
    chron(2, { name: "Factures du mois", cron: "0 7 1 * *", kind: "monthly_invoices", enabled: false, next_run: null, description: "Le 1er du mois, 07:00" }),
  ]);
  api.cronDescribe.mockImplementation(async (e: string) =>
    e === "0 9 * * *" ? ok("Tous les jours, 09:00") : { ok: false, description: null, next_runs: [], error: "Expression cron invalide : 5 champs attendus." });
});

describe("ChronsScreen", () => {
  it("liste périodicité, cron en monospace, prochain passage et interrupteur", async () => {
    render(ChronsScreen);
    expect(await screen.findByText("Chron 1")).toBeTruthy();
    expect(screen.getByText("Jours ouvrés, 08:00")).toBeTruthy();
    expect(screen.getByText("0 8 * * 1-5").tagName).toBe("CODE");
    expect(screen.getByText(/^Prochain passage : /)).toBeTruthy();
    expect(screen.getByText("En pause")).toBeTruthy();
    const sw = screen.getAllByRole("switch");
    expect(sw.map((s) => s.getAttribute("aria-checked"))).toEqual(["true", "false"]);
    await fireEvent.click(sw[1]);
    await waitFor(() => expect(api.chronToggle).toHaveBeenCalledWith(2, true));
  });

  it("valide l'expression en direct : erreur puis périodicité en clair, bouton selon validité", async () => {
    render(ChronsScreen);
    await screen.findByText("Chron 1");
    await fireEvent.click(screen.getByRole("button", { name: "Nouveau chron" }));
    const save = screen.getByRole("button", { name: "Enregistrer" }) as HTMLButtonElement;
    await fireEvent.input(screen.getByLabelText(/Nom/), { target: { value: "Matin" } });
    await fireEvent.input(screen.getByLabelText(/Expression cron/), { target: { value: "0 8 *" } });
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("5 champs attendus"), { timeout: 2000 });
    expect(save.disabled).toBe(true);

    await fireEvent.input(screen.getByLabelText(/Expression cron/), { target: { value: "0 9 * * *" } });
    await waitFor(() => expect(screen.getByText("Tous les jours, 09:00")).toBeTruthy(), { timeout: 2000 });
    expect(screen.getAllByText(/nov/).length).toBeGreaterThanOrEqual(3);
    expect(save.disabled).toBe(false);
    expect(api.cronDescribe).toHaveBeenLastCalledWith("0 9 * * *");
  });

  it("enregistre le nouveau chron puis recharge la liste", async () => {
    api.chronSave.mockImplementation(async (c: Chron) => ({ ...c, id: 5 }));
    render(ChronsScreen);
    await screen.findByText("Chron 1");
    await fireEvent.click(screen.getByRole("button", { name: "Nouveau chron" }));
    await fireEvent.input(screen.getByLabelText(/Nom/), { target: { value: "Matin" } });
    await fireEvent.input(screen.getByLabelText(/Expression cron/), { target: { value: "0 9 * * *" } });
    const save = screen.getByRole("button", { name: "Enregistrer" }) as HTMLButtonElement;
    await waitFor(() => expect(save.disabled).toBe(false), { timeout: 2000 });
    await fireEvent.click(save);
    await waitFor(() => expect(api.chronSave).toHaveBeenCalled());
    expect(api.chronSave.mock.calls[0][0]).toMatchObject({ id: null, name: "Matin", cron: "0 9 * * *", kind: "morning_digest", enabled: true });
    await waitFor(() => expect(api.chronsList).toHaveBeenCalledTimes(2));
  });

  it("Modifier ouvre l'éditeur sur le chron et l'erreur du backend s'affiche", async () => {
    api.cronDescribe.mockResolvedValue(ok("Jours ouvrés, 08:00"));
    api.chronSave.mockRejectedValue("Expression refusée.");
    render(ChronsScreen);
    await screen.findByText("Chron 1");
    await fireEvent.click(screen.getAllByRole("button", { name: "Modifier" })[0]);
    expect((screen.getByLabelText(/Nom/) as HTMLInputElement).value).toBe("Chron 1");
    const save = screen.getByRole("button", { name: "Enregistrer" }) as HTMLButtonElement;
    await waitFor(() => expect(save.disabled).toBe(false), { timeout: 2000 });
    await fireEvent.click(save);
    expect(await screen.findByText("Expression refusée.")).toBeTruthy();
  });

  it("Supprimer demande confirmation", async () => {
    render(ChronsScreen);
    await screen.findByText("Chron 1");
    await fireEvent.click(screen.getAllByRole("button", { name: "Supprimer" })[0]);
    expect(api.chronDelete).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "Confirmer la suppression" }));
    await waitFor(() => expect(api.chronDelete).toHaveBeenCalledWith(1));
  });
});

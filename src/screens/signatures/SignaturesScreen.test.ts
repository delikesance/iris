import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { get } from "svelte/store";

const api = vi.hoisted(() => ({
  documentsList: vi.fn(), documentGet: vi.fn(), documentRemind: vi.fn(), proofGet: vi.fn(), account: vi.fn(), signatureImageGet: vi.fn(),
  signApply: vi.fn(), signerAdd: vi.fn(), signerRemove: vi.fn(), documentExportForReply: vi.fn(), documentImport: vi.fn(),
}));
vi.mock("../../lib/api", () => api);
vi.mock("../../components/signing/pdf", () => ({
  fetchPdfBytes: vi.fn(async () => new Uint8Array([1])),
  openPdf: vi.fn(async () => ({ numPages: 1, pageSize: async () => ({ width: 600, height: 800 }), render: async () => {}, destroy: () => {} })),
}));

import SignaturesScreen from "./SignaturesScreen.svelte";
import { composeDraft, toast } from "../../lib/stores";

const mk = (id: number, name: string, status: string, signers: [string, string, boolean][]) => ({
  id, name, path: `/${name}`, sha256: "", status, level: "simple", origin_message_id: null, created: 1_700_000_000, signable: true,
  signers: signers.map(([n, e, s]) => ({ name: n, email: e, signed: s })),
});
const todo = mk(1, "Contrat v3.pdf", "todo", [["Moi", "me@x.fr", false], ["Camille", "c@x.fr", false]]);
const waiting = mk(2, "Avenant.pdf", "waiting", [["Moi", "me@x.fr", true], ["Lea", "l@x.fr", false]]);
const signed = mk(3, "NDA.pdf", "signed", [["Moi", "me@x.fr", true], ["Paul", "p@x.fr", true]]);

afterEach(cleanup);
beforeEach(() => {
  vi.clearAllMocks();
  composeDraft.set(null); toast.set(null);
  api.documentsList.mockImplementation(async (t: string) => ({ todo: [todo], waiting: [waiting], signed: [signed] } as Record<string, unknown[]>)[t]);
  api.account.mockResolvedValue("me@x.fr");
  api.signatureImageGet.mockResolvedValue("PNG");
  api.documentGet.mockImplementation(async (id: number) => [todo, waiting, signed].find(d => d.id === id));
  api.proofGet.mockResolvedValue({ document: todo, events: [{ id: 1, at: 1_700_000_000, kind: "opened", detail: "Document consulté par vous" }] });
  api.documentExportForReply.mockResolvedValue("/signed.pdf");
});

describe("SignaturesScreen", () => {
  it("onglets avec compteurs, ligne, 1 sur 2 et action Signer", async () => {
    render(SignaturesScreen);
    expect(await screen.findByText("Contrat v3.pdf")).toBeTruthy();
    for (const l of ["À signer 1", "En attente 1", "Signés 1"]) expect(screen.getByRole("tab", { name: l })).toBeTruthy();
    expect(screen.getByText("0 sur 2")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Signer" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "+ Nouveau document" })).toBeTruthy();
  });

  it("En attente : 1 sur 2 + Relancer appelle document_remind", async () => {
    render(SignaturesScreen);
    await fireEvent.click(await screen.findByRole("tab", { name: "En attente 1" }));
    expect(screen.getByText("1 sur 2")).toBeTruthy();
    expect(screen.getByText("En attente", { selector: "span" })).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Relancer" }));
    await waitFor(() => expect(api.documentRemind).toHaveBeenCalledWith(2));
    expect(get(toast)?.kind).toBe("info");
  });

  it("Signés : action Télécharger", async () => {
    render(SignaturesScreen);
    await fireEvent.click(await screen.findByRole("tab", { name: "Signés 1" }));
    expect(screen.getByRole("button", { name: "Télécharger" })).toBeTruthy();
    expect(screen.getByText("2 sur 2")).toBeTruthy();
  });

  it("Signer ouvre le détail : niveaux désactivés, envoi désactivé avant signature, journal", async () => {
    render(SignaturesScreen);
    await fireEvent.click(await screen.findByRole("button", { name: "Signer" }));
    expect(await screen.findByText("Signataires")).toBeTruthy();
    expect(screen.getByText("Vous")).toBeTruthy();
    expect(screen.getByText("Camille")).toBeTruthy();
    expect((screen.getByRole("button", { name: "Avancée" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Qualifiée" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Simple" }) as HTMLButtonElement).disabled).toBe(false);
    expect(screen.getByText(/prestataire de services de confiance/)).toBeTruthy();
    expect((screen.getByRole("button", { name: /Renvoyer à Camille/ }) as HTMLButtonElement).disabled).toBe(true);
    expect(await screen.findByText(/Document consulté par vous/)).toBeTruthy();
  });

  it("détail : signer active l'envoi ; ajout et retrait de signataire", async () => {
    api.signApply.mockResolvedValue({ ...todo, status: "waiting", signers: [{ name: "Moi", email: "me@x.fr", signed: true }, todo.signers[1]] });
    api.signerAdd.mockResolvedValue({ ...todo, signers: [...todo.signers, { name: "Zoe", email: "z@x.fr", signed: false }] });
    api.signerRemove.mockResolvedValue(todo);
    render(SignaturesScreen);
    await fireEvent.click(await screen.findByRole("button", { name: "Signer" }));
    await fireEvent.click(await screen.findByRole("button", { name: "Ajouter un signataire" }));
    await fireEvent.input(screen.getByLabelText("Nom"), { target: { value: "Zoe" } });
    await fireEvent.input(screen.getByLabelText("Adresse e-mail"), { target: { value: "z@x.fr" } });
    await fireEvent.click(screen.getByRole("button", { name: "Ajouter" }));
    await waitFor(() => expect(api.signerAdd).toHaveBeenCalledWith(1, "Zoe", "z@x.fr"));
    await fireEvent.click(await screen.findByRole("button", { name: "Retirer Zoe" }));
    await waitFor(() => expect(api.signerRemove).toHaveBeenCalledWith(1, "z@x.fr"));

    await fireEvent.click(await screen.findByLabelText("Champ de signature"), { clientX: 50, clientY: 50 });
    await waitFor(() => expect(api.signApply).toHaveBeenCalled());
    const send = (await screen.findByRole("button", { name: /Renvoyer à Camille/ })) as HTMLButtonElement;
    await waitFor(() => expect(send.disabled).toBe(false));
    await fireEvent.click(send);
    await waitFor(() => expect(get(composeDraft)?.attachments).toEqual(["/signed.pdf"]));
  });
});

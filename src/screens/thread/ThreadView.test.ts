import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, fireEvent, waitFor } from "@testing-library/svelte";
import { get } from "svelte/store";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: vi.fn(), open: vi.fn() }));

import ThreadView from "./ThreadView.svelte";
import { composeDraft, signingTarget } from "../../lib/stores";

const att = (index: number, filename: string, extra = {}) => ({ index, filename, mime: "application/pdf", size: 1, signable: true, ...extra });
const base = { folder: "INBOX", subject: "Contrat de prestation, clause 4", html: null, superseded: false };
const list = [
  { ...base, uid: 1, from_name: "Camille", from_addr: "camille@x.fr", to: "moi@iris.fr", date: 1_761_735_600, message_id: "<a>", text: "Bonjour",
    attachments: [att(0, "Contrat v2.pdf", { superseded_by: "Contrat v3.pdf" }), att(1, "Contrat v3.pdf")] },
  { ...base, uid: 2, from_name: "Moi", from_addr: "moi@iris.fr", to: "camille@x.fr", date: 1_761_742_800, message_id: "<b>", text: "Merci", attachments: [] },
];

afterEach(cleanup);
beforeEach(() => {
  composeDraft.set(null); signingTarget.set(null);
  invoke.mockReset();
  invoke.mockImplementation(async (cmd: string) => (cmd === "thread_get" ? list : "moi@iris.fr"));
});

describe("ThreadView", () => {
  it("affiche fil, pièce remplacée, un seul Signer, pastille et Répondre à tous", async () => {
    render(ThreadView, { threadId: "t1", reminderDays: 3 });
    await screen.findByText("2 messages · 2 participants");
    expect(screen.getByText("Remplacée par la v3, non signable")).toBeTruthy();
    expect(screen.getAllByRole("button", { name: "Signer" })).toHaveLength(1);
    expect(screen.getByText("Relance automatique dans 3 jours sans réponse")).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Répondre à tous" }));
    expect(get(composeDraft)).toMatchObject({ to: "camille@x.fr", in_reply_to: "<b>", subject: "Re : Contrat de prestation, clause 4" });
  });

  it("Signer renseigne signingTarget", async () => {
    render(ThreadView, { threadId: "t1" });
    await fireEvent.click(await screen.findByRole("button", { name: "Signer" }));
    expect(get(signingTarget)).toEqual({ folder: "INBOX", uid: 1, index: 1 });
    expect(await screen.findByLabelText("Fermer l'aperçu")).toBeTruthy();
  });

  it("un message se replie", async () => {
    render(ThreadView, { threadId: "t1" });
    const head = (await screen.findAllByRole("button", { expanded: true }))[0];
    await fireEvent.click(head);
    await waitFor(() => expect(screen.queryByText("Bonjour")).toBeNull());
  });
});

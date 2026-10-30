import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { get } from "svelte/store";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: async () => () => {} }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: async () => "/tmp/x.pdf" }));

import MailScreen from "./MailScreen.svelte";
import { composeDraft, currentFolder, mailQuery, route, selectedContact, selectedUid, selection, signingTarget } from "../../lib/stores";
import type { Folder, Full, MessageRow } from "../../lib/types";

const now = Math.floor(Date.now() / 1000);
const folders: Folder[] = [
  { name: "INBOX", label: "Réception", kind: "system", unread: 2 },
  { name: "Archive", label: "Archives", kind: "system", unread: 0 },
  { name: "Clients", label: "Clients", kind: "sorted", unread: 6 },
];
const rows: MessageRow[] = [
  { uid: 1, folder: "INBOX", from_name: "Léa Martin", from_addr: "lea@x.fr", subject: "Devis atelier", date: now, seen: false, has_attach: true, thread_id: null, rule_id: 4, rule_name: "Semaine client", sorted_into: "Clients" },
  { uid: 2, folder: "INBOX", from_name: "Thomas Roux", from_addr: "t@x.fr", subject: "Notes de point", date: now - 60, seen: true, has_attach: false, thread_id: null, rule_id: null, rule_name: null, sorted_into: null },
];
const full: Full = {
  uid: 1, folder: "INBOX", from: "Léa Martin <lea@x.fr>", from_name: "Léa Martin", from_addr: "lea@x.fr", to: "moi", cc: "", subject: "Devis atelier",
  date: now, message_id: "<m1@x.fr>", thread_id: null, text: "Bonjour", html: "<p>Bonjour</p>",
  attachments: [
    { index: 0, filename: "devis.pdf", mime: "application/pdf", size: 284672, signable: true },
    { index: 1, filename: "plan.png", mime: "image/png", size: 1468006, signable: false },
  ],
  rule_id: 4, rule_name: "Semaine client", sorted_into: "Clients",
};

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(async (cmd: string) => {
    if (cmd === "list_folders") return folders;
    if (cmd === "list_messages") return rows;
    if (cmd === "get_message") return full;
    return null;
  });
  currentFolder.set("INBOX");
  selectedUid.set(null);
  selection.set(new Set());
  composeDraft.set(null);
  signingTarget.set(null);
  route.set("mail");
});
afterEach(cleanup);

describe("MailScreen", () => {
  it("shows folders with counters, day group, unread dot and sorted tag", async () => {
    render(MailScreen);
    await screen.findByText("Devis atelier");
    expect(screen.getByText("Triés pour vous")).toBeTruthy();
    expect(screen.getByLabelText("6 non lus")).toBeTruthy();
    expect(screen.getByText("Aujourd'hui")).toBeTruthy();
    expect(screen.getAllByRole("img", { name: "Non lu" })).toHaveLength(1);
    expect(screen.getAllByText("Clients").length).toBeGreaterThan(1);
  });

  it("opens the reader: sandboxed iframe, rule note, attachments, signing pencil", async () => {
    const { container } = render(MailScreen);
    await fireEvent.click(await screen.findByText("Devis atelier"));
    await screen.findByText(/Classé dans Clients par la règle/);
    expect(container.querySelector("iframe")?.getAttribute("sandbox")).toBe("");
    expect(screen.getByLabelText("Télécharger devis.pdf")).toBeTruthy();
    expect(screen.queryByLabelText("Signer plan.png")).toBeNull();
    await fireEvent.click(screen.getByLabelText("Signer devis.pdf"));
    expect(get(signingTarget)).toEqual({ folder: "INBOX", uid: 1, index: 0 });
    expect(invoke).toHaveBeenCalledWith("mark_seen", { folder: "INBOX", uid: 1, seen: true });
  });

  it("links: sender → contacts, Modifier → rule, Répondre → draft", async () => {
    const { container } = render(MailScreen);
    await fireEvent.click(await screen.findByText("Devis atelier"));
    await fireEvent.click(await screen.findByText("Modifier"));
    expect(get(route)).toBe("settings/rules/4");
    await fireEvent.click(container.querySelector("article .sender")!);
    expect(get(route)).toBe("contacts");
    expect(get(selectedContact)).toMatchObject({ name: "Léa Martin", email: "lea@x.fr" });
    await fireEvent.click(screen.getByText("Répondre"));
    expect(get(composeDraft)).toMatchObject({ to: "lea@x.fr", subject: "Re : Devis atelier", in_reply_to: "<m1@x.fr>" });
  });

  it("Ctrl+A selects every visible message; the filter panel narrows the list", async () => {
    render(MailScreen);
    await screen.findByText("Devis atelier");
    await fireEvent.keyDown(window, { key: "a", ctrlKey: true });
    expect(get(selection).size).toBe(2);
    await fireEvent.click(screen.getByText("Filtrer"));
    await fireEvent.click(screen.getByText("Non lus"));
    await waitFor(() => expect(screen.queryByText("Notes de point")).toBeNull());
    expect(screen.getByText("Devis atelier")).toBeTruthy();
  });

  it("Voir le fil ouvre la route thread", async () => {
    const withThread = { ...full, thread_id: "t/1" };
    invoke.mockImplementation(async (cmd: string) => (cmd === "list_folders" ? folders : cmd === "list_messages" ? rows : cmd === "get_message" ? withThread : null));
    render(MailScreen);
    await fireEvent.click(await screen.findByText("Devis atelier"));
    await fireEvent.click(await screen.findByText("Voir le fil"));
    expect(get(route)).toBe("thread/t%2F1");
  });

  it("monte SigningPanel quand signingTarget est posé, et la fermeture le vide", async () => {
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_folders") return folders;
      if (cmd === "list_messages") return rows;
      if (cmd === "get_message") return full;
      if (cmd === "signature_image_get") return null;
      return new Promise(() => {});
    });
    render(MailScreen);
    await fireEvent.click(await screen.findByText("Devis atelier"));
    await fireEvent.click(await screen.findByLabelText("Signer devis.pdf"));
    await fireEvent.click(await screen.findByLabelText("Fermer l'aperçu"));
    expect(get(signingTarget)).toBeNull();
  });

  it("applique mailQuery à l'ouverture puis le consomme", async () => {
    mailQuery.set("lea@x.fr");
    render(MailScreen);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("list_messages", expect.objectContaining({ query: "lea@x.fr" })));
    expect(get(mailQuery)).toBe("");
  });
});

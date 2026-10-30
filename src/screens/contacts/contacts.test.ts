import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { get } from "svelte/store";

const api = vi.hoisted(() => ({
  contactsList: vi.fn(), contactGet: vi.fn(), contactSave: vi.fn(),
  ruleSentence: vi.fn(), threadGet: vi.fn(),
}));
vi.mock("../../lib/api", () => api);

import ContactsScreen from "./ContactsScreen.svelte";
import { draftTo, formatDay, validContact } from "./contacts";
import { composeDraft, currentFolder, mailQuery, route, selectedContact, selectedUid, toast } from "../../lib/stores";
import type { Contact, ContactDetail } from "../../lib/types";

const camille: Contact = { id: 1, name: "Camille Bernard", email: "camille@atelier-norge.fr", notes: "Atelier Norge" };
const detail: ContactDetail = {
  contact: camille,
  recent_threads: [{ thread_id: "t1", subject: "Contrat de prestation", date: 1_700_000_000 }],
  rule: { id: 7, name: "Norge", enabled: true, position: 0, cond: { type: "sender_domain", value: "atelier-norge.fr" }, action: { type: "notify" } },
};

describe("contacts helpers", () => {
  it("draftTo / validContact / formatDay", () => {
    expect(draftTo("a@b.fr")).toEqual({ to: "a@b.fr", subject: "", body: "", attachments: [] });
    expect(validContact("A", "a@b.fr")).toBe(true);
    expect(validContact("", "a@b.fr")).toBe(false);
    expect(validContact("A", "a@b")).toBe(false);
    const now = new Date(2026, 9, 30, 12);
    expect(formatDay(new Date(2026, 9, 30, 8).getTime() / 1000, now)).toBe("Aujourd'hui");
    expect(formatDay(new Date(2026, 9, 29, 8).getTime() / 1000, now)).toBe("Hier");
  });
});

describe("ContactsScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.contactsList.mockResolvedValue([camille]);
    api.contactGet.mockResolvedValue(detail);
    api.ruleSentence.mockResolvedValue("Les mails du domaine atelier-norge.fr sont classés dans Clients.");
    route.set("contacts"); composeDraft.set(null); toast.set(null);
  });
  afterEach(cleanup);

  async function openCamille() {
    render(ContactsScreen);
    await fireEvent.click(await screen.findByText("Camille Bernard"));
    await screen.findByText("Écrire");
  }

  it("shows the fiche with threads and rule sentence", async () => {
    await openCamille();
    expect(screen.getByText("Contrat de prestation")).toBeTruthy();
    expect(await screen.findByText(/classés dans Clients/)).toBeTruthy();
  });

  it("Écrire opens a draft to the contact", async () => {
    await openCamille();
    await fireEvent.click(screen.getByText("Écrire"));
    expect(get(composeDraft)?.to).toBe("camille@atelier-norge.fr");
  });

  it("thread opens the last message in the mail route", async () => {
    api.threadGet.mockResolvedValue([{ folder: "INBOX", uid: 1 }, { folder: "Clients", uid: 9 }]);
    await openCamille();
    await fireEvent.click(screen.getByText("Contrat de prestation"));
    await waitFor(() => expect(get(route)).toBe("mail"));
    expect(get(currentFolder)).toBe("Clients");
    expect(get(selectedUid)).toBe(9);
  });

  it("Voir tous les mails pose mailQuery et ouvre la route mail", async () => {
    mailQuery.set("");
    await openCamille();
    await fireEvent.click(screen.getByText("Voir tous les mails"));
    expect(get(mailQuery)).toBe("camille@atelier-norge.fr");
    expect(get(route)).toBe("mail");
  });

  it("présélectionne selectedContact puis le vide", async () => {
    selectedContact.set(camille);
    render(ContactsScreen);
    await screen.findByText("Écrire");
    expect(api.contactGet).toHaveBeenCalledWith("camille@atelier-norge.fr");
    expect(get(selectedContact)).toBeNull();
  });

  it("Modifier goes to the rule settings", async () => {
    await openCamille();
    await fireEvent.click(await screen.findByText("Modifier"));
    expect(get(route)).toBe("settings/rules/7");
  });

  it("search queries the backend", async () => {
    render(ContactsScreen);
    await screen.findByText("Camille Bernard");
    await fireEvent.input(screen.getByLabelText("Rechercher un contact"), { target: { value: "cam" } });
    await waitFor(() => expect(api.contactsList).toHaveBeenLastCalledWith("cam"));
  });

  it("Nouveau creates a contact and opens it", async () => {
    api.contactSave.mockResolvedValue(camille);
    render(ContactsScreen);
    await fireEvent.click(await screen.findByText("Nouveau"));
    expect((screen.getByText("Enregistrer") as HTMLButtonElement).disabled).toBe(true);
    await fireEvent.input(screen.getByLabelText("Nom"), { target: { value: "Camille Bernard" } });
    await fireEvent.input(screen.getByLabelText("Adresse e-mail"), { target: { value: "camille@atelier-norge.fr" } });
    await fireEvent.click(screen.getByText("Enregistrer"));
    await waitFor(() => expect(api.contactSave).toHaveBeenCalledWith({ id: null, name: "Camille Bernard", email: "camille@atelier-norge.fr", notes: null }));
    await screen.findByText("Voir tous les mails");
  });
});

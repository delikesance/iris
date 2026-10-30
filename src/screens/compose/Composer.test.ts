import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, fireEvent, waitFor } from "@testing-library/svelte";
import { get } from "svelte/store";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
const open = vi.fn();
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: (...a: unknown[]) => open(...a), save: vi.fn() }));

import Composer from "./Composer.svelte";
import { composeDraft } from "../../lib/stores";

afterEach(cleanup);
beforeEach(() => {
  invoke.mockReset(); open.mockReset();
  invoke.mockResolvedValue({ status: "sent", id: null });
  composeDraft.set({ to: "lea@x.fr", subject: "Re : Devis", body: "Bonjour", attachments: ["/d/Devis (signé).pdf"] });
});

describe("Composer", () => {
  it("rien quand composeDraft est null", () => {
    composeDraft.set(null);
    render(Composer);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("chip signée et envoi immédiat", async () => {
    render(Composer);
    expect(screen.getByText("✓ signé par vous, avec journal de preuve")).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Envoyer maintenant" }));
    await waitFor(() => expect(invoke).toHaveBeenCalled());
    expect(invoke).toHaveBeenCalledWith("send_mail", { draft: expect.objectContaining({ to: "lea@x.fr", send_at: null, remind_days: null, attachments: ["/d/Devis (signé).pdf"] }) });
    await waitFor(() => expect(get(composeDraft)).toBeNull());
  });

  it("créneau + relance : bouton Planifier et send_at / remind_days", async () => {
    render(Composer);
    await fireEvent.click(screen.getByRole("button", { name: "Envoyer plus tard" }));
    await fireEvent.click(screen.getByRole("button", { name: /^Demain matin/ }));
    await fireEvent.click(screen.getByRole("switch", { name: "Relancer sans réponse" }));
    await fireEvent.click(screen.getByRole("button", { name: /^Planifier pour demain, 08:30/ }));
    await waitFor(() => expect(invoke).toHaveBeenCalled());
    const draft = invoke.mock.calls[0][1].draft;
    expect(typeof draft.send_at).toBe("number");
    expect(new Date(draft.send_at * 1000).getHours()).toBe(8);
    expect(draft.remind_days).toBe(3);
  });

  it("joindre un fichier via le dialogue", async () => {
    open.mockResolvedValue(["/d/a.pdf"]);
    render(Composer);
    await fireEvent.click(screen.getByRole("button", { name: "Joindre un fichier" }));
    await screen.findByText("a.pdf");
  });
});

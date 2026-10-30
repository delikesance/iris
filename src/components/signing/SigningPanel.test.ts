import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { get } from "svelte/store";

const api = vi.hoisted(() => ({
  sofficeAvailable: vi.fn(), signatureImageGet: vi.fn(), signPrepareAttachment: vi.fn(), signApply: vi.fn(), documentExportForReply: vi.fn(),
}));
vi.mock("../../lib/api", () => api);
vi.mock("./pdf", () => ({
  fetchPdfBytes: vi.fn(async () => new Uint8Array([1])),
  openPdf: vi.fn(async () => ({
    numPages: 1, pageSize: async () => ({ width: 600, height: 800 }), render: async () => {}, destroy: () => {},
  })),
}));

import SigningPanel from "./SigningPanel.svelte";
import { composeDraft, route } from "../../lib/stores";
import { openDocumentId } from "./state";

const base = { id: 7, name: "c.pdf", path: "/c.pdf", sha256: "", status: "todo", signers: [], level: "simple", origin_message_id: null, created: 0, signable: true };
const props = { target: { folder: "INBOX", uid: 3, index: 0 }, filename: "c.pdf", replyTo: "camille@x.fr", subject: "Contrat", inReplyTo: "<m1>" };

afterEach(cleanup);
beforeEach(() => {
  vi.clearAllMocks();
  composeDraft.set(null);
  api.sofficeAvailable.mockResolvedValue(true);
  api.signatureImageGet.mockResolvedValue("PNGB64");
  api.signPrepareAttachment.mockResolvedValue(base);
  api.signApply.mockResolvedValue({ ...base, status: "waiting" });
  api.documentExportForReply.mockResolvedValue("/signed.pdf");
});

describe("SigningPanel", () => {
  it("Word sans soffice : message explicatif, aucun import", async () => {
    api.sofficeAvailable.mockResolvedValue(false);
    render(SigningPanel, { props: { ...props, filename: "c.docx" } });
    expect((await screen.findByRole("alert")).textContent).toContain("LibreOffice");
    expect(api.signPrepareAttachment).not.toHaveBeenCalled();
  });

  it("clic sur le champ -> sign_apply (page 0-based), puis réponse avec le PDF signé", async () => {
    render(SigningPanel, { props });
    const reply = screen.getByRole("button", { name: /Répondre avec le document signé/ }) as HTMLButtonElement;
    expect(reply.disabled).toBe(true);
    const hit = await screen.findByLabelText("Champ de signature");
    await fireEvent.click(hit, { clientX: 100, clientY: 100 });
    await waitFor(() => expect(api.signApply).toHaveBeenCalled());
    const [id, page, rect, png] = api.signApply.mock.calls[0];
    expect([id, page, png]).toEqual([7, 0, "PNGB64"]);
    expect(rect).toHaveLength(4);
    await waitFor(() => expect(reply.disabled).toBe(false));
    await fireEvent.click(reply);
    await waitFor(() => expect(get(composeDraft)).toMatchObject({ to: "camille@x.fr", subject: "Re : Contrat", attachments: ["/signed.pdf"], in_reply_to: "<m1>" }));
  });

  it("sans image de signature : invite à la saisir, champ non cliquable", async () => {
    api.signatureImageGet.mockResolvedValue(null);
    render(SigningPanel, { props });
    expect(await screen.findByText(/Aucune signature manuscrite/)).toBeTruthy();
    expect(screen.queryByLabelText("Champ de signature")).toBeNull();
  });

  it("Voir le détail ouvre la page détail", async () => {
    render(SigningPanel, { props });
    const b = screen.getByRole("button", { name: "Voir le détail" }) as HTMLButtonElement;
    await waitFor(() => expect(b.disabled).toBe(false));
    await fireEvent.click(b);
    expect(get(openDocumentId)).toBe(7);
    expect(get(route)).toBe("signatures");
  });
});

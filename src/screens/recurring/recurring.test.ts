import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { get } from "svelte/store";

const api = vi.hoisted(() => ({
  newSendersList: vi.fn(), recurringList: vi.fn(), senderDecide: vi.fn(), unsubscribe: vi.fn(),
}));
const openUrl = vi.hoisted(() => vi.fn());
vi.mock("../../lib/api", () => api);
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl }));

import RecurringScreen from "./RecurringScreen.svelte";
import { groupRows, tabItems, unreadLabel } from "./recurring";
import { toast } from "../../lib/stores";
import type { RecurringSender } from "../../lib/types";

const row = (o: Partial<RecurringSender>): RecurringSender => ({
  addr: "a@x.fr", name: "A", kind: "newsletters", cadence: "Hebdo", last_subject: "S",
  unread: 0, filed_in: null, can_unsubscribe: true, ...o,
});
const rows = [
  row({ addr: "n1@x.fr", name: "La Lettre Pro" }),
  row({ addr: "n2@x.fr", name: "Journal Tech", unread: 4 }),
  row({ addr: "p1@x.fr", name: "Agenda Pro", kind: "notifications" }),
  row({ addr: "d1@x.fr", name: "Facturation Norge", kind: "documents", filed_in: "Factures", can_unsubscribe: false, unread: 1 }),
];

describe("recurring helpers", () => {
  it("counts per tab and groups in fixed order", () => {
    expect(tabItems(rows).map(t => t.label)).toEqual(["Tous 4", "Newsletters 2", "Notifications 1", "Documents 1"]);
    expect(groupRows(rows, "all").map(g => g.kind)).toEqual(["newsletters", "notifications", "documents"]);
    expect(groupRows(rows, "documents")).toHaveLength(1);
    expect(groupRows([], "all")).toEqual([]);
  });
  it("pluralises unread", () => {
    expect([unreadLabel(1), unreadLabel(4)]).toEqual(["1 non lu", "4 non lus"]);
  });
});

describe("RecurringScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.newSendersList.mockResolvedValue([{ addr: "n@lumen.fr", name: "Atelier Lumen", last_subject: "Bienvenue", count: 1 }]);
    api.recurringList.mockResolvedValue(rows);
    api.senderDecide.mockResolvedValue(undefined);
    toast.set(null);
  });
  afterEach(cleanup);

  it("renders new senders and rows, filters by tab", async () => {
    render(RecurringScreen);
    await screen.findByText("Atelier Lumen");
    expect(screen.getAllByTestId("recurring-row")).toHaveLength(4);
    expect(screen.getByText("Classé dans Factures")).toBeTruthy();
    expect(screen.getAllByText("Se désinscrire")).toHaveLength(3);
    await fireEvent.click(screen.getByRole("tab", { name: "Notifications 1" }));
    expect(screen.getAllByTestId("recurring-row")).toHaveLength(1);
  });

  it("sends the decision then reloads", async () => {
    render(RecurringScreen);
    await fireEvent.click(await screen.findByText("Vers Récurrents"));
    await waitFor(() => expect(api.senderDecide).toHaveBeenCalledWith("n@lumen.fr", "recurring"));
    await waitFor(() => expect(api.newSendersList).toHaveBeenCalledTimes(2));
  });

  it("asks for confirmation, then opens the unsubscribe url", async () => {
    api.unsubscribe.mockResolvedValue({ done: false, open_url: "https://x.fr/unsub" });
    render(RecurringScreen);
    await screen.findByText("La Lettre Pro");
    await fireEvent.click(screen.getAllByText("Se désinscrire")[0]);
    expect(api.unsubscribe).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByText("Confirmer"));
    await waitFor(() => expect(openUrl).toHaveBeenCalledWith("https://x.fr/unsub"));
    expect(api.unsubscribe).toHaveBeenCalledWith("n1@x.fr");
    expect(get(toast)?.kind).toBe("info");
  });

  it("cancel leaves the sender untouched; backend error becomes a toast", async () => {
    api.unsubscribe.mockRejectedValue("échec");
    render(RecurringScreen);
    await screen.findByText("La Lettre Pro");
    await fireEvent.click(screen.getAllByText("Se désinscrire")[0]);
    await fireEvent.click(screen.getByText("Annuler"));
    expect(api.unsubscribe).not.toHaveBeenCalled();
    await fireEvent.click(screen.getAllByText("Se désinscrire")[0]);
    await fireEvent.click(screen.getByText("Confirmer"));
    await waitFor(() => expect(get(toast)).toEqual({ kind: "error", text: "échec" }));
    expect(openUrl).not.toHaveBeenCalled();
  });
});

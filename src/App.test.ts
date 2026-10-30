import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/svelte";

const msg = { uid: 1, folder: "INBOX", from_name: "Camille", from_addr: "c@x.fr", to: "moi@iris.fr", subject: "Fil de test", date: 1_761_735_600, message_id: "<a>", text: "Bonjour", html: null, attachments: [], superseded: false };
const invoke = vi.fn(async (cmd: string, args?: { threadId?: string }) => (cmd === "thread_get" ? (args?.threadId === "t1" ? [msg] : []) : "moi@iris.fr"));
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...(a as [string, never])) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: async () => () => {} }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: vi.fn(), open: vi.fn() }));

import App from "./App.svelte";
import { route } from "./lib/stores";
import { threadRoute } from "./lib/router";

afterEach(cleanup);

describe("App", () => {
  it("monte ThreadView sur la route thread avec son identifiant", async () => {
    route.set(threadRoute("t1"));
    render(App);
    expect(await screen.findByText("Fil de test")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("thread_get", { threadId: "t1" });
  });
});

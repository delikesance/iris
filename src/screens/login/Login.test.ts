import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import Login from "./Login.svelte";

vi.mock("../../lib/api", () => ({ login: vi.fn() }));

describe("Login", () => {
  afterEach(cleanup);
  it("shows the default servers on one line, then IMAP/SMTP fields after Modifier", async () => {
    render(Login, { onsuccess: () => {} });
    expect(screen.getByText("imap.fastmail.com · smtp.fastmail.com")).toBeTruthy();
    expect(screen.queryByText("Réception (IMAP)")).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: /Modifier/ }));
    expect(screen.getByText("Réception (IMAP)")).toBeTruthy();
    expect(screen.getByText("Envoi (SMTP)")).toBeTruthy();
  });

  it("changing IMAP security to STARTTLS moves the port to 143", async () => {
    const { container } = render(Login, { onsuccess: () => {} });
    await fireEvent.click(screen.getByRole("button", { name: /Modifier/ }));
    const sel = container.querySelector<HTMLSelectElement>("select[name=imap_security]")!;
    await fireEvent.change(sel, { target: { value: "starttls" } });
    expect(container.querySelector<HTMLInputElement>("input[name=imap_port]")!.value).toBe("143");
  });
});

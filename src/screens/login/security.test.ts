import { describe, expect, it } from "vitest";
import { defaultPort, defaultSettings, portAfterSecurityChange, serversSummary, validPort } from "./security";

describe("security -> port defaults", () => {
  it("IMAP defaults", () => {
    expect(defaultPort("imap", "ssl")).toBe(993);
    expect(defaultPort("imap", "starttls")).toBe(143);
    expect(defaultPort("imap", "none")).toBe(143);
  });
  it("SMTP defaults", () => {
    expect(defaultPort("smtp", "ssl")).toBe(465);
    expect(defaultPort("smtp", "starttls")).toBe(587);
    expect(defaultPort("smtp", "none")).toBe(25);
  });
  it("default port follows the security change", () => {
    expect(portAfterSecurityChange("imap", "ssl", "starttls", 993)).toBe(143);
    expect(portAfterSecurityChange("smtp", "ssl", "starttls", 465)).toBe(587);
    expect(portAfterSecurityChange("smtp", "starttls", "ssl", 587)).toBe(465);
  });
  it("custom port is kept", () => {
    expect(portAfterSecurityChange("imap", "ssl", "starttls", 1993)).toBe(1993);
  });
  it("default settings are Fastmail over SSL", () => {
    const s = defaultSettings("a@b.fr");
    expect(s).toMatchObject({ imap_host: "imap.fastmail.com", imap_port: 993, smtp_host: "smtp.fastmail.com", smtp_port: 465, imap_security: "ssl", smtp_security: "ssl" });
    expect(serversSummary(s)).toBe("imap.fastmail.com · smtp.fastmail.com");
  });
  it("validPort", () => {
    expect([validPort(993), validPort(0), validPort(70000), validPort(1.5)]).toEqual([true, false, false, false]);
  });
});

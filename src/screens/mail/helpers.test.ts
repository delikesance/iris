import { describe, expect, it } from "vitest";
import type { Full, MessageRow } from "../../lib/types";
import { applyFilters, formatSize, groupByDay, messageSrcdoc, replyDraft, replySubject } from "./helpers";

const ts = (y: number, mo: number, d: number, h = 12, mi = 0) => Math.floor(new Date(y, mo - 1, d, h, mi).getTime() / 1000);

const row = (uid: number, over: Partial<MessageRow> = {}): MessageRow => ({
  uid, folder: "INBOX", from_name: "Léa Martin", from_addr: "lea@atelier-norge.fr", subject: `Sujet ${uid}`,
  date: ts(2026, 10, 30), seen: true, has_attach: false, thread_id: null, rule_id: null, rule_name: null, sorted_into: null,
  ...over,
});

describe("groupByDay", () => {
  const now = new Date(2026, 9, 30, 15, 0);

  it("labels today, yesterday and older days, newest first", () => {
    const g = groupByDay(
      [row(1, { date: ts(2026, 10, 28) }), row(2, { date: ts(2026, 10, 30, 9) }), row(3, { date: ts(2026, 10, 29, 22) })],
      now,
    );
    expect(g.map((x) => x.label)).toEqual(["Aujourd'hui", "Hier", "28 octobre 2026"]);
    expect(g.map((x) => x.items[0].uid)).toEqual([2, 3, 1]);
  });

  it("keeps same-day messages together, sorted by time desc", () => {
    const g = groupByDay([row(1, { date: ts(2026, 10, 30, 8) }), row(2, { date: ts(2026, 10, 30, 11) })], now);
    expect(g).toHaveLength(1);
    expect(g[0].items.map((m) => m.uid)).toEqual([2, 1]);
  });

  it("splits at local midnight", () => {
    const g = groupByDay([row(1, { date: ts(2026, 10, 30, 0, 5) }), row(2, { date: ts(2026, 10, 29, 23, 55) })], now);
    expect(g.map((x) => x.label)).toEqual(["Aujourd'hui", "Hier"]);
  });

  it("yesterday works across a month boundary", () => {
    const g = groupByDay([row(1, { date: ts(2026, 10, 31, 10) })], new Date(2026, 10, 1, 9));
    expect(g[0].label).toBe("Hier");
  });

  it("returns [] for no messages and does not mutate the input", () => {
    expect(groupByDay([], now)).toEqual([]);
    const input = [row(1, { date: ts(2026, 10, 1) }), row(2, { date: ts(2026, 10, 2) })];
    groupByDay(input, now);
    expect(input.map((m) => m.uid)).toEqual([1, 2]);
  });
});

describe("applyFilters", () => {
  const rows = [
    row(1, { seen: false }),
    row(2, { has_attach: true, subject: "Facture 2026-114" }),
    row(3, { rule_id: 4, rule_name: "Clients", sorted_into: "Clients", from_name: "Thomas Roux", from_addr: "thomas@roux.fr" }),
    row(4, { sorted_into: "Factures" }),
  ];
  const uids = (m: MessageRow[]) => m.map((x) => x.uid);

  it("all keeps everything", () => expect(uids(applyFilters(rows, { mode: "all", query: "" }))).toEqual([1, 2, 3, 4]));
  it("unread", () => expect(uids(applyFilters(rows, { mode: "unread", query: "" }))).toEqual([1]));
  it("attach", () => expect(uids(applyFilters(rows, { mode: "attach", query: "" }))).toEqual([2]));
  it("sorted = filed by a rule or in a sorted folder", () =>
    expect(uids(applyFilters(rows, { mode: "sorted", query: "" }))).toEqual([3, 4]));

  it("query matches subject, name and address, ignoring case and accents", () => {
    expect(uids(applyFilters(rows, { mode: "all", query: "FACTURE" }))).toEqual([2]);
    expect(uids(applyFilters(rows, { mode: "all", query: "roux" }))).toEqual([3]);
    expect(uids(applyFilters(rows, { mode: "all", query: "lea" }))).toEqual([1, 2, 4]);
  });

  it("combines mode and query", () => {
    expect(uids(applyFilters(rows, { mode: "attach", query: "roux" }))).toEqual([]);
    expect(uids(applyFilters(rows, { mode: "sorted", query: "roux" }))).toEqual([3]);
  });
});

describe("reply and format helpers", () => {
  const full = {
    uid: 9, folder: "INBOX", from: "Léa <lea@x.fr>", from_name: "Léa Martin", from_addr: "lea@x.fr", to: "me@x.fr", cc: "",
    subject: "Devis", date: ts(2026, 10, 30, 9, 42), message_id: "<m1@x.fr>", thread_id: null, text: "Bonjour\nMerci",
    html: null, attachments: [], rule_id: null, rule_name: null, sorted_into: null,
  } satisfies Full;

  it("replySubject does not stack prefixes", () => {
    expect(replySubject("Devis")).toBe("Re : Devis");
    expect(replySubject("RE: Devis")).toBe("RE: Devis");
  });

  it("replyDraft targets the sender, quotes the text and keeps the thread", () => {
    const d = replyDraft(full, "Bien reçu.");
    expect(d.to).toBe("lea@x.fr");
    expect(d.in_reply_to).toBe("<m1@x.fr>");
    expect(d.body.startsWith("Bien reçu.\n\nLe ")).toBe(true);
    expect(d.body).toContain("> Bonjour\n> Merci");
  });

  it("formatSize", () => {
    expect(formatSize(900)).toBe("900 o");
    expect(formatSize(284672)).toBe("278 Ko");
    expect(formatSize(1468006)).toBe("1,4 Mo");
  });

  it("srcdoc forbids scripts and network, and escapes plain text", () => {
    const html = messageSrcdoc({ html: "<p>x</p>", text: "" });
    expect(html).toContain("default-src 'none'");
    const plain = messageSrcdoc({ html: null, text: "<script>alert(1)</script>" });
    expect(plain).not.toContain("<script>");
    expect(plain).toContain("&lt;script&gt;");
  });
});

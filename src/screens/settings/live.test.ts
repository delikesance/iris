import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { liveCheck } from "./live";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("liveCheck", () => {
  it("n'appelle qu'une fois après une rafale et rend la dernière valeur", async () => {
    const fn = vi.fn(async (s: string) => s.toUpperCase());
    const out: string[] = [];
    const live = liveCheck(fn, (r) => out.push(r), () => {}, 100);
    live.run("a"); live.run("ab"); live.run("abc");
    await vi.advanceTimersByTimeAsync(100);
    expect(fn).toHaveBeenCalledTimes(1);
    expect(out).toEqual(["ABC"]);
  });

  it("ignore une réponse périmée", async () => {
    let release!: (v: string) => void;
    const fn = vi.fn((s: string) => (s === "slow" ? new Promise<string>((r) => (release = r)) : Promise.resolve(s)));
    const out: string[] = [];
    const live = liveCheck(fn, (r) => out.push(r), () => {}, 10);
    live.run("slow");
    await vi.advanceTimersByTimeAsync(10);
    live.run("fast");
    await vi.advanceTimersByTimeAsync(10);
    release("slow");
    await vi.advanceTimersByTimeAsync(0);
    expect(out).toEqual(["fast"]);
  });

  it("remonte l'erreur en chaîne, et cancel() annule l'appel prévu", async () => {
    const errors: string[] = [];
    const live = liveCheck(async () => { throw "boum"; }, () => {}, (e) => errors.push(e), 10);
    live.run(1);
    await vi.advanceTimersByTimeAsync(10);
    expect(errors).toEqual(["boum"]);
    const fn = vi.fn(async () => 1);
    const l2 = liveCheck(fn, () => {}, () => {}, 10);
    l2.run(1); l2.cancel();
    await vi.advanceTimersByTimeAsync(50);
    expect(fn).not.toHaveBeenCalled();
  });
});

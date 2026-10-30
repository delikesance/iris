import { describe, expect, it } from "vitest";
import { PREFIX, toBase64 } from "./signatureData";

describe("toBase64", () => {
  it("retire l'en-tête data URL", () => expect(toBase64(`${PREFIX}QUJD`)).toBe("QUJD"));
  it("refuse autre chose qu'un PNG", () => expect(() => toBase64("data:image/jpeg;base64,QUJD")).toThrow());
});

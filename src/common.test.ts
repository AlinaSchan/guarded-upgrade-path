import { describe, expect, it } from "vitest";
import { fmt, pct, shorten } from "./components/common";

describe("display helpers", () => {
  it("shortens a commitment in the style of the review draft", () => {
    const h = "0x7c1e" + "ab".repeat(28) + "a94d";
    expect(shorten(h)).toBe("0x7c1e…a94d");
    expect(shorten("0x1234")).toBe("0x1234");
    expect(shorten(null)).toBe("—");
  });

  it("formats basis points as a percentage", () => {
    expect(pct(30)).toBe("0.30%");
    expect(pct(250)).toBe("2.50%");
  });

  it("formats numbers with separators", () => {
    expect(fmt(41200)).toBe("41,200");
    expect(fmt(null)).toBe("—");
  });
});

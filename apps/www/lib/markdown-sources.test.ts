import { describe, expect, it } from "vitest";
import { extractMarkdownSources } from "./markdown-sources";

describe("extractMarkdownSources", () => {
  it("collects markdown links and bare URLs without duplicates", () => {
    const sources = extractMarkdownSources(
      "See [Stripe](https://stripe.com/docs) and https://stripe.com/docs plus https://react.dev.",
    );
    expect(sources).toEqual([
      { href: "https://stripe.com/docs", title: "Stripe" },
      { href: "https://react.dev", title: "react.dev" },
    ]);
  });

  it("returns nothing when there are no http links", () => {
    expect(extractMarkdownSources("No sources here, just /local/path.md")).toEqual([]);
  });
});

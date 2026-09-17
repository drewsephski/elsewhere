import { describe, expect, test } from "vitest";
import { CloudApiError } from "./cloud-api-error";
import { formatUserFacingError } from "./format-api-error";

describe("formatUserFacingError", () => {
  test("appends request id for CloudApiError", () => {
    const error = new CloudApiError("Runner unavailable", {
      status: 503,
      requestId: "req-abc-123",
    });
    expect(formatUserFacingError(error)).toBe("Runner unavailable (ref: req-abc-123)");
  });

  test("omits ref when request id is missing", () => {
    const error = new CloudApiError("Not found", { status: 404 });
    expect(formatUserFacingError(error)).toBe("Not found");
  });

  test("uses Error message for plain errors", () => {
    expect(formatUserFacingError(new Error("Network failed"))).toBe("Network failed");
  });
});

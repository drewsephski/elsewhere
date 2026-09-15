import { describe, expect, it } from "vitest";
import { CloudApiError, cloudApiErrorFromResponse } from "./cloud-api-error";

describe("CloudApiError", () => {
  it("detects runner unreachable from structured BFF body", async () => {
    const response = new Response(
      JSON.stringify({
        code: "workspace_upstream_unreachable",
        error: "Workspace runner is temporarily unreachable.",
        retryable: true,
        requestId: "req-1",
      }),
      { status: 503 },
    );
    const error = await cloudApiErrorFromResponse(response, "fallback");
    expect(error).toBeInstanceOf(CloudApiError);
    expect(error.code).toBe("workspace_upstream_unreachable");
    expect(error.runnerUnreachable).toBe(true);
    expect(error.requestId).toBe("req-1");
  });
});

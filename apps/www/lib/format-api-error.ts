import { isCloudApiError } from "@/lib/cloud-api-error";

/** User-visible error text; includes support reference when the BFF provides a request id. */
export function formatUserFacingError(error: unknown, fallback = "Something went wrong"): string {
  if (isCloudApiError(error)) {
    const ref = error.requestId?.trim();
    return ref ? `${error.message} (ref: ${ref})` : error.message;
  }
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }
  return fallback;
}

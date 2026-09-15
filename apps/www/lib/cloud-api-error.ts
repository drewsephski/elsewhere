import {
  type CloudApiErrorBody,
  isRunnerUnreachableStatus,
  readCloudApiErrorBody,
} from "@/lib/cloud-bff-errors";

export class CloudApiError extends Error {
  readonly code: string | undefined;
  readonly status: number;
  readonly retryable: boolean;
  readonly phase: string | undefined;
  readonly requestId: string | undefined;

  constructor(
    message: string,
    options: {
      status: number;
      code?: string;
      retryable?: boolean;
      phase?: string;
      requestId?: string;
    },
  ) {
    super(message);
    this.name = "CloudApiError";
    this.status = options.status;
    this.code = options.code;
    this.retryable = options.retryable ?? false;
    this.phase = options.phase;
    this.requestId = options.requestId;
  }

  get runnerUnreachable(): boolean {
    return isRunnerUnreachableStatus(this.status, {
      code: this.code,
      error: this.message,
      retryable: this.retryable,
      requestId: this.requestId,
    });
  }
}

export async function cloudApiErrorFromResponse(
  response: Response,
  fallback: string,
): Promise<CloudApiError> {
  const body: CloudApiErrorBody | null = await readCloudApiErrorBody(response.clone());
  const message =
    body?.error?.trim() || body?.message?.trim() || `${fallback} (${response.status})`;
  return new CloudApiError(message, {
    status: response.status,
    code: body?.code,
    retryable: body?.retryable,
    phase: body?.phase,
    requestId: body?.requestId,
  });
}

export function isCloudApiError(error: unknown): error is CloudApiError {
  return error instanceof CloudApiError;
}

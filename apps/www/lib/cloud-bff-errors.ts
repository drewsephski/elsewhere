export type CloudApiErrorBody = {
  code?: string;
  error?: string;
  message?: string;
  retryable?: boolean;
  requestId?: string;
  attempted?: string[];
};

export async function readCloudApiErrorBody(response: Response): Promise<CloudApiErrorBody | null> {
  const text = await response.text().catch(() => "");
  if (!text) {
    return null;
  }
  try {
    return JSON.parse(text) as CloudApiErrorBody;
  } catch {
    return null;
  }
}

export function isRunnerUnreachableStatus(
  status: number,
  body: CloudApiErrorBody | null,
): boolean {
  return status === 503 && body?.code === "workspace_upstream_unreachable";
}

import { cloudHostFetch } from "@/lib/cloud-api";

export interface WorkspaceMutationResult {
  path: string;
  revision: number;
}

export async function deleteWorkspaceEntry(
  computerId: string,
  path: string,
): Promise<WorkspaceMutationResult> {
  const response = await cloudHostFetch(
    `/v1/computers/${encodeURIComponent(computerId)}/workspace/file?path=${encodeURIComponent(path)}`,
    { method: "DELETE" },
  );
  const body = await response.json().catch(() => ({}));
  if (!response.ok) {
    const message =
      typeof body.error === "string" ? body.error : "Could not delete this item";
    throw new Error(message);
  }
  return body as WorkspaceMutationResult;
}

export async function renameWorkspaceEntry(
  computerId: string,
  path: string,
  newName: string,
): Promise<WorkspaceMutationResult> {
  const response = await cloudHostFetch(
    `/v1/computers/${encodeURIComponent(computerId)}/workspace/file`,
    {
      method: "PATCH",
      body: JSON.stringify({ path, newName }),
    },
  );
  const body = await response.json().catch(() => ({}));
  if (!response.ok) {
    const message =
      typeof body.error === "string" ? body.error : "Could not rename this item";
    throw new Error(message);
  }
  return body as WorkspaceMutationResult;
}

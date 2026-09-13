import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

export const vmLifecycleStateSchema = z.enum([
  "notCreated",
  "stopped",
  "starting",
  "running",
  "stopping",
  "error",
]);

export type VmLifecycleState = z.infer<typeof vmLifecycleStateSchema>;

export const vmInfoSchema = z.object({
  state: vmLifecycleStateSchema,
  hostArch: z.string(),
  guestArch: z.string(),
  cpuCount: z.number(),
  memoryMib: z.number(),
  diskPath: z.string(),
  diskBytes: z.number(),
  configPath: z.string(),
  guestBridgeReady: z.boolean(),
  message: z.string().nullable().optional(),
  created: z.boolean(),
});

export type VmInfo = z.infer<typeof vmInfoSchema>;

export const guestResponseSchema = z.object({
  id: z.string(),
  ok: z.boolean(),
  stdout: z.string().nullable().optional(),
  stderr: z.string().nullable().optional(),
  exitCode: z.number().nullable().optional(),
  error: z.string().nullable().optional(),
  protocolVersion: z.number().nullable().optional(),
});

export type GuestResponse = z.infer<typeof guestResponseSchema>;

export const vmService = {
  async info(): Promise<VmInfo> {
    const raw = await invoke<unknown>("vm_info");
    return vmInfoSchema.parse(raw);
  },
  async provision(): Promise<VmInfo> {
    const raw = await invoke<unknown>("vm_provision");
    return vmInfoSchema.parse(raw);
  },
  async start(): Promise<VmInfo> {
    const raw = await invoke<unknown>("vm_start");
    return vmInfoSchema.parse(raw);
  },
  async stop(): Promise<VmInfo> {
    const raw = await invoke<unknown>("vm_stop");
    return vmInfoSchema.parse(raw);
  },
  async restart(): Promise<VmInfo> {
    const raw = await invoke<unknown>("vm_restart");
    return vmInfoSchema.parse(raw);
  },
  async guestHealth(): Promise<boolean> {
    return invoke<boolean>("vm_guest_health");
  },
  async guestRequest(input: {
    id: string;
    method: string;
    params: Record<string, unknown>;
  }): Promise<GuestResponse> {
    const raw = await invoke<unknown>("vm_guest_request", { request: input });
    return guestResponseSchema.parse(raw);
  },
};

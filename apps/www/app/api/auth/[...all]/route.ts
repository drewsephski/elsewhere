import { auth } from "@/lib/auth";
import { ensureAuthSchema } from "@/lib/db";
import { toNextJsHandler } from "better-auth/next-js";

const { GET: authGet, POST: authPost } = toNextJsHandler(auth);

export async function GET(request: Request) {
  await ensureAuthSchema();
  return authGet(request);
}

export async function POST(request: Request) {
  await ensureAuthSchema();
  return authPost(request);
}

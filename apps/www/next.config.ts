import path from "node:path";
import { fileURLToPath } from "node:url";
import type { NextConfig } from "next";

const wwwRoot = path.dirname(fileURLToPath(import.meta.url));

const nextConfig: NextConfig = {
  transpilePackages: ["@gptbot/brand"],
  outputFileTracingRoot: path.join(wwwRoot, "../.."),
};

export default nextConfig;

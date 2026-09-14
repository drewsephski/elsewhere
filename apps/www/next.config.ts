import path from "node:path";
import { fileURLToPath } from "node:url";
import type { NextConfig } from "next";

const wwwRoot = path.dirname(fileURLToPath(import.meta.url));

const nextConfig: NextConfig = {
  distDir: process.env.NEXT_DIST_DIR ?? ".next",
  transpilePackages: ["@elsewhere/brand"],
  outputFileTracingRoot: path.join(wwwRoot, "../.."),
};

export default nextConfig;

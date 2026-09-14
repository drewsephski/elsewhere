import fs from "fs";
import dns from "dns/promises";
import { chromium } from "playwright-core";

export const BROWSER_ROOT = "/var/elsewhere/browser";
export const PROFILE = `${BROWSER_ROOT}/profile`;
export const SOCKET_PATH = `${BROWSER_ROOT}/daemon.sock`;
export const REFS_PATH = `${BROWSER_ROOT}/refs.json`;

export const PLAYWRIGHT_VERSION = "1.49.1";
export const NODE_VERSION = "20.18.0";

export const MAX_SNAPSHOT_ELEMENTS = 100;
export const MAX_SNAPSHOT_LINE_CHARS = 120;
export const MAX_DOWNLOAD_BYTES = 10 * 1024 * 1024;
export const MAX_TYPE_TEXT_CHARS = 8_192;
export const MAX_NAVIGATION_TIMEOUT_MS = 120_000;
export const MAX_ACTION_TIMEOUT_MS = 30_000;
export const MAX_PREVIEW_BYTES = 512 * 1024;
export const MAX_DOWNLOAD_REDIRECTS = 10;

const BLOCKED_HOSTNAMES = new Set([
  "localhost",
  "metadata.google.internal",
  "metadata.goog",
]);

const CHROMIUM_CANDIDATES = [
  process.env.CHROMIUM_PATH,
  "/usr/bin/chromium-browser",
  "/usr/bin/chromium",
  "/usr/bin/google-chrome-stable",
].filter(Boolean);

export function findChromium() {
  for (const candidate of CHROMIUM_CANDIDATES) {
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }
  return undefined;
}

export function launchOptions() {
  const executablePath = findChromium();
  const base = {
    headless: true,
    args: ["--no-sandbox", "--disable-dev-shm-usage", "--disable-gpu"],
  };
  if (executablePath) {
    return { ...base, executablePath };
  }
  return base;
}

function isPrivateIpv4(host) {
  const m = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.exec(host);
  if (!m) {
    return false;
  }
  const [a, b] = [Number(m[1]), Number(m[2])];
  if (a === 10) return true;
  if (a === 127) return true;
  if (a === 0) return true;
  if (a === 169 && b === 254) return true;
  if (a === 172 && b >= 16 && b <= 31) return true;
  if (a === 192 && b === 168) return true;
  if (a === 100 && b >= 64 && b <= 127) return true;
  return false;
}

function isBlockedIpv6(host) {
  const normalized = host.toLowerCase();
  return (
    normalized === "::1" ||
    normalized.startsWith("fc") ||
    normalized.startsWith("fd") ||
    normalized.startsWith("fe80")
  );
}

function assertLiteralHostAllowed(host, label) {
  if (BLOCKED_HOSTNAMES.has(host) || host.endsWith(".localhost")) {
    throw new Error(`${label} targets a blocked host`);
  }
  if (isPrivateIpv4(host) || isBlockedIpv6(host)) {
    throw new Error(`${label} targets a private or link-local address`);
  }
  if (host === "169.254.169.254") {
    throw new Error(`${label} targets cloud metadata`);
  }
}

async function assertResolvedHostAllowed(hostname, label) {
  const v4 = await dns.resolve4(hostname).catch(() => []);
  const v6 = await dns.resolve6(hostname).catch(() => []);
  const all = [...v4, ...v6];
  if (all.length === 0) {
    throw new Error(`${label} host could not be resolved`);
  }
  for (const addr of all) {
    assertLiteralHostAllowed(addr.toLowerCase(), label);
  }
}

export async function assertPublicHttpUrl(rawUrl, label = "url") {
  let parsed;
  try {
    parsed = new URL(rawUrl);
  } catch {
    throw new Error(`${label} is not a valid URL`);
  }
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
    throw new Error(`${label} must use http or https`);
  }
  const host = parsed.hostname.toLowerCase();
  assertLiteralHostAllowed(host, label);
  if (!isPrivateIpv4(host) && !isBlockedIpv6(host)) {
    const isIpLiteral = /^\d{1,3}(\.\d{1,3}){3}$/.test(host) || host.includes(":");
    if (!isIpLiteral) {
      await assertResolvedHostAllowed(host, label);
    }
  }
  return parsed.toString();
}

export function requireWorkspacePath(filePath) {
  if (filePath === "/workspace" || filePath.startsWith("/workspace/")) {
    return filePath;
  }
  throw new Error(`path must be under /workspace, got ${filePath}`);
}

export async function installRequestGuards(context) {
  await context.route("**/*", async (route) => {
    try {
      await assertPublicHttpUrl(route.request().url(), "request");
      await route.continue();
    } catch {
      await route.abort("blockedbyclient");
    }
  });
}

export async function downloadHttpWithRedirects(context, startUrl) {
  let url = await assertPublicHttpUrl(String(startUrl ?? ""), "url");
  for (let hop = 0; hop <= MAX_DOWNLOAD_REDIRECTS; hop += 1) {
    const response = await context.request.get(url, {
      timeout: MAX_NAVIGATION_TIMEOUT_MS,
      maxRedirects: 0,
    });
    const status = response.status();
    if (status >= 300 && status < 400) {
      const location = response.headers().location || response.headers().Location;
      if (!location) {
        throw new Error("redirect response missing Location header");
      }
      url = await assertPublicHttpUrl(new URL(location, url).toString(), "redirect");
      continue;
    }
    if (!response.ok()) {
      throw new Error(`download failed: HTTP ${status}`);
    }
    const body = await response.body();
    if (body.length > MAX_DOWNLOAD_BYTES) {
      throw new Error(`download exceeds ${MAX_DOWNLOAD_BYTES} bytes`);
    }
    return body;
  }
  throw new Error("download exceeded redirect limit");
}

export async function buildSnapshot(page) {
  const selectors =
    "a, button, input, textarea, select, [role=button], [role=link], [href]";
  const elements = await page.locator(selectors).all();
  const lines = [];
  let counter = 0;
  for (const el of elements) {
    if (counter >= MAX_SNAPSHOT_ELEMENTS) {
      break;
    }
    const visible = await el.isVisible().catch(() => false);
    if (!visible) {
      continue;
    }
    counter += 1;
    const ref = `e${counter}`;
    await el.evaluate(
      (node, r) => node.setAttribute("data-elsewhere-ref", r),
      ref,
    );
    const tag = await el.evaluate((node) => node.tagName.toLowerCase());
    const text = ((await el.innerText().catch(() => "")) || "")
      .trim()
      .slice(0, MAX_SNAPSHOT_LINE_CHARS);
    const placeholder = await el.getAttribute("placeholder").catch(() => null);
    const name = await el.getAttribute("name").catch(() => null);
    const aria = await el.getAttribute("aria-label").catch(() => null);
    const label = text || placeholder || name || aria || tag;
    lines.push(`- ${ref} [${tag}] ${label}`);
  }
  fs.mkdirSync(BROWSER_ROOT, { recursive: true });
  fs.writeFileSync(REFS_PATH, JSON.stringify({ count: counter }));
  const truncated = counter >= MAX_SNAPSHOT_ELEMENTS;
  return {
    url: page.url(),
    title: await page.title(),
    snapshot: lines.join("\n") || "(no interactive elements visible)",
    elementCount: counter,
    truncated,
  };
}

export async function resolveRef(page, ref) {
  const loc = page.locator(`[data-elsewhere-ref="${ref}"]`);
  if ((await loc.count()) === 0) {
    throw new Error(`unknown ref ${ref}; call browser_snapshot first`);
  }
  return loc.first();
}

export async function launchPersistentContext() {
  fs.mkdirSync(PROFILE, { recursive: true });
  const context = await chromium.launchPersistentContext(PROFILE, {
    ...launchOptions(),
    acceptDownloads: true,
  });
  await installRequestGuards(context);
  return context;
}

import { chromium } from "playwright-core";
import fs from "fs";
import path from "path";

const PROFILE = "/workspace/.elsewhere/browser/profile";
const REFS_PATH = "/workspace/.elsewhere/browser/refs.json";
const SESSION_PATH = "/workspace/.elsewhere/browser/session.json";

const CHROMIUM_CANDIDATES = [
  process.env.CHROMIUM_PATH,
  "/usr/bin/chromium-browser",
  "/usr/bin/chromium",
  "/usr/bin/google-chrome-stable",
].filter(Boolean);

function findChromium() {
  for (const candidate of CHROMIUM_CANDIDATES) {
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }
  return undefined;
}

function launchOptions() {
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

function readRequest() {
  const fileFlag = process.argv.indexOf("--request");
  if (fileFlag !== -1 && process.argv[fileFlag + 1]) {
    return JSON.parse(fs.readFileSync(process.argv[fileFlag + 1], "utf8"));
  }
  if (process.argv[2]) {
    return JSON.parse(process.argv[2]);
  }
  throw new Error("missing request JSON (--request path or argv[2])");
}

function saveSession(data) {
  fs.mkdirSync(path.dirname(SESSION_PATH), { recursive: true });
  fs.writeFileSync(SESSION_PATH, JSON.stringify(data));
}

function loadSession() {
  try {
    return JSON.parse(fs.readFileSync(SESSION_PATH, "utf8"));
  } catch {
    return {};
  }
}

async function ensureActivePage(page) {
  const session = loadSession();
  const current = page.url();
  if (
    session.url &&
    (current === "about:blank" || current === "chrome://newtab/")
  ) {
    await page.goto(session.url, {
      waitUntil: "domcontentloaded",
      timeout: 120_000,
    });
  }
}

function requireWorkspacePath(filePath) {
  if (filePath === "/workspace" || filePath.startsWith("/workspace/")) {
    return filePath;
  }
  throw new Error(`path must be under /workspace, got ${filePath}`);
}

async function buildSnapshot(page) {
  const selectors =
    "a, button, input, textarea, select, [role=button], [role=link], [href]";
  const elements = await page.locator(selectors).all();
  const lines = [];
  let counter = 0;
  for (const el of elements) {
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
      .slice(0, 120);
    const placeholder = await el.getAttribute("placeholder").catch(() => null);
    const name = await el.getAttribute("name").catch(() => null);
    const aria = await el.getAttribute("aria-label").catch(() => null);
    const label = text || placeholder || name || aria || tag;
    lines.push(`- ${ref} [${tag}] ${label}`);
  }
  fs.mkdirSync(path.dirname(REFS_PATH), { recursive: true });
  fs.writeFileSync(REFS_PATH, JSON.stringify({ count: counter }));
  return {
    url: page.url(),
    title: await page.title(),
    snapshot: lines.join("\n") || "(no interactive elements visible)",
    elementCount: counter,
  };
}

async function resolveRef(page, ref) {
  const loc = page.locator(`[data-elsewhere-ref="${ref}"]`);
  if ((await loc.count()) === 0) {
    throw new Error(`unknown ref ${ref}; call browser_snapshot first`);
  }
  return loc.first();
}

async function main() {
  const req = readRequest();
  const action = req.action;

  fs.mkdirSync(PROFILE, { recursive: true });
  const context = await chromium.launchPersistentContext(PROFILE, {
    ...launchOptions(),
    acceptDownloads: true,
  });

  try {
    const pages = context.pages();
    const page = pages.length ? pages[0] : await context.newPage();
    await ensureActivePage(page);
    let result;

    switch (action) {
      case "navigate": {
        if (!req.url || typeof req.url !== "string") {
          throw new Error("url is required");
        }
        await page.goto(req.url, {
          waitUntil: "domcontentloaded",
          timeout: 120_000,
        });
        saveSession({ url: page.url() });
        result = {
          ok: true,
          url: page.url(),
          title: await page.title(),
        };
        break;
      }
      case "snapshot": {
        result = { ok: true, ...(await buildSnapshot(page)) };
        saveSession({ url: page.url() });
        break;
      }
      case "click": {
        await ensureActivePage(page);
        const target = await resolveRef(page, req.ref);
        await target.click({ timeout: 30_000 });
        result = {
          ok: true,
          url: page.url(),
          title: await page.title(),
        };
        break;
      }
      case "type": {
        await ensureActivePage(page);
        const target = await resolveRef(page, req.ref);
        await target.fill(String(req.text ?? ""));
        if (req.submit) {
          await target.press("Enter");
        }
        result = { ok: true, url: page.url() };
        break;
      }
      case "screenshot": {
        await ensureActivePage(page);
        const outPath = requireWorkspacePath(String(req.path ?? ""));
        fs.mkdirSync(path.dirname(outPath), { recursive: true });
        await page.screenshot({
          path: outPath,
          fullPage: req.fullPage !== false,
        });
        const stat = fs.statSync(outPath);
        result = { ok: true, path: outPath, bytes: stat.size };
        break;
      }
      case "download": {
        const outPath = requireWorkspacePath(String(req.path ?? ""));
        if (!req.url || typeof req.url !== "string") {
          throw new Error("url is required");
        }
        fs.mkdirSync(path.dirname(outPath), { recursive: true });
        const response = await context.request.get(req.url, { timeout: 120_000 });
        if (!response.ok()) {
          throw new Error(`download failed: HTTP ${response.status()}`);
        }
        const body = await response.body();
        fs.writeFileSync(outPath, body);
        result = { ok: true, path: outPath, bytes: body.length };
        break;
      }
      default:
        throw new Error(`unknown browser action: ${action}`);
    }

    process.stdout.write(`${JSON.stringify(result)}\n`);
  } finally {
    await context.close();
  }
}

main().catch((err) => {
  process.stderr.write(`${err.message || String(err)}\n`);
  process.exit(1);
});

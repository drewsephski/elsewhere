import net from "net";
import fs from "fs";
import path from "path";
import {
  BROWSER_ROOT,
  SOCKET_PATH,
  MAX_ACTION_TIMEOUT_MS,
  MAX_DOWNLOAD_BYTES,
  MAX_NAVIGATION_TIMEOUT_MS,
  MAX_TYPE_TEXT_CHARS,
  MAX_PREVIEW_BYTES,
  assertPublicHttpUrl,
  buildSnapshot,
  downloadHttpWithRedirects,
  launchPersistentContext,
  requireWorkspacePath,
  resolveRef,
} from "./browser-common.mjs";
import {
  loadPreviewVersionFromDisk,
  readPreviewCacheFromDisk,
  writePreviewCacheAtomic,
} from "./browser-preview-cache.mjs";

let contextPromise;
let shuttingDown = false;

const PREVIEW_DIR = path.join(BROWSER_ROOT, "preview");
let previewVersion = loadPreviewVersionFromDisk(PREVIEW_DIR);

function persistPreviewCache(metaFields, imageBuffer) {
  const result = writePreviewCacheAtomic(
    PREVIEW_DIR,
    previewVersion,
    metaFields,
    imageBuffer,
  );
  previewVersion = result.version;
}

async function refreshPreviewCache(page) {
  const currentUrl = page.url();
  if (!currentUrl || currentUrl === "about:blank") {
    try {
      persistPreviewCache(
        {
          available: false,
          url: currentUrl || null,
          title: null,
        },
        null,
      );
    } catch {
      // best-effort cache update
    }
    return;
  }
  const buffer = await page.screenshot({
    type: "jpeg",
    quality: 62,
    fullPage: false,
    timeout: MAX_ACTION_TIMEOUT_MS,
  });
  if (buffer.length > MAX_PREVIEW_BYTES) {
    throw new Error("preview frame exceeds size limit");
  }
  persistPreviewCache(
    {
      available: true,
      url: currentUrl,
      title: await page.title(),
    },
    buffer,
  );
}

/** Serialize all daemon RPCs so concurrent MCP calls cannot race page/refs. */
let requestTail = Promise.resolve();

function runExclusive(task) {
  const next = requestTail.then(() => task());
  requestTail = next.catch(() => {});
  return next;
}

async function getContext() {
  if (!contextPromise) {
    contextPromise = launchPersistentContext().catch((err) => {
      contextPromise = undefined;
      throw err;
    });
  }
  return contextPromise;
}

async function resetContext() {
  if (contextPromise) {
    try {
      const ctx = await contextPromise;
      await ctx.close();
    } catch {
      // ignore close errors during restart
    }
    contextPromise = undefined;
  }
}

async function activePage() {
  const context = await getContext();
  const pages = context.pages();
  const page = pages.length ? pages[0] : await context.newPage();
  return { context, page };
}

async function handleRequest(req) {
  const action = req.action;
  if (action === "health") {
    return { ok: true, pid: process.pid };
  }
  if (action === "shutdown") {
    shuttingDown = true;
    await resetContext();
    return { ok: true };
  }

  const { context, page } = await activePage();

  switch (action) {
    case "navigate": {
      const url = await assertPublicHttpUrl(String(req.url ?? ""), "url");
      await page.goto(url, {
        waitUntil: "domcontentloaded",
        timeout: MAX_NAVIGATION_TIMEOUT_MS,
      });
      await refreshPreviewCache(page);
      return {
        ok: true,
        url: page.url(),
        title: await page.title(),
      };
    }
    case "snapshot": {
      const snap = await buildSnapshot(page);
      await refreshPreviewCache(page);
      return { ok: true, ...snap };
    }
    case "click": {
      const target = await resolveRef(page, req.ref);
      await target.click({ timeout: MAX_ACTION_TIMEOUT_MS });
      await refreshPreviewCache(page);
      return {
        ok: true,
        url: page.url(),
        title: await page.title(),
      };
    }
    case "click_point": {
      const xRatio = Number(req.xRatio);
      const yRatio = Number(req.yRatio);
      if (!Number.isFinite(xRatio) || !Number.isFinite(yRatio)) {
        throw new Error("xRatio and yRatio must be numbers");
      }
      if (xRatio < 0 || xRatio > 1 || yRatio < 0 || yRatio > 1) {
        throw new Error("xRatio and yRatio must be between 0 and 1");
      }
      const viewport = page.viewportSize() ?? { width: 1280, height: 720 };
      await page.mouse.click(xRatio * viewport.width, yRatio * viewport.height, {
        timeout: MAX_ACTION_TIMEOUT_MS,
      });
      await refreshPreviewCache(page);
      return {
        ok: true,
        url: page.url(),
        title: await page.title(),
      };
    }
    case "press": {
      const key = String(req.key ?? "").trim();
      if (!key) {
        throw new Error("key is required");
      }
      await page.keyboard.press(key);
      await refreshPreviewCache(page);
      return { ok: true, url: page.url() };
    }
    case "type": {
      const text = String(req.text ?? "");
      if (text.length > MAX_TYPE_TEXT_CHARS) {
        throw new Error(`text exceeds ${MAX_TYPE_TEXT_CHARS} characters`);
      }
      const target = await resolveRef(page, req.ref);
      await target.fill(text);
      if (req.submit) {
        await target.press("Enter");
      }
      await refreshPreviewCache(page);
      return { ok: true, url: page.url() };
    }
    case "preview": {
      return readPreviewCacheFromDisk(PREVIEW_DIR);
    }
    case "screenshot": {
      const outPath = requireWorkspacePath(String(req.path ?? ""));
      fs.mkdirSync(path.dirname(outPath), { recursive: true });
      await page.screenshot({
        path: outPath,
        fullPage: req.fullPage !== false,
        timeout: MAX_ACTION_TIMEOUT_MS,
      });
      const stat = fs.statSync(outPath);
      if (stat.size > MAX_DOWNLOAD_BYTES) {
        fs.unlinkSync(outPath);
        throw new Error("screenshot exceeds size limit");
      }
      return { ok: true, path: outPath, bytes: stat.size };
    }
    case "download": {
      const outPath = requireWorkspacePath(String(req.path ?? ""));
      fs.mkdirSync(path.dirname(outPath), { recursive: true });
      const body = await downloadHttpWithRedirects(
        context,
        String(req.url ?? ""),
      );
      fs.writeFileSync(outPath, body);
      return { ok: true, path: outPath, bytes: body.length };
    }
    default:
      throw new Error(`unknown browser action: ${action}`);
  }
}

function writeResponse(socket, payload) {
  socket.write(`${JSON.stringify(payload)}\n`);
}

async function handleConnection(socket) {
  let buffer = "";
  socket.setEncoding("utf8");
  socket.on("data", async (chunk) => {
    buffer += chunk;
    let newline;
    while ((newline = buffer.indexOf("\n")) !== -1) {
      const line = buffer.slice(0, newline).trim();
      buffer = buffer.slice(newline + 1);
      if (!line) {
        continue;
      }
      try {
        const req = JSON.parse(line);
        const result = await runExclusive(() => handleRequest(req));
        writeResponse(socket, result);
      } catch (err) {
        writeResponse(socket, {
          ok: false,
          error: err.message || String(err),
        });
        if (
          err.message?.includes("Target closed") ||
          err.message?.includes("Browser has been closed")
        ) {
          await resetContext();
        }
      }
      if (shuttingDown) {
        socket.end();
      }
    }
  });
}

fs.mkdirSync(BROWSER_ROOT, { recursive: true, mode: 0o700 });
try {
  fs.unlinkSync(SOCKET_PATH);
} catch {
  // ignore missing socket
}

const server = net.createServer(handleConnection);
server.listen(SOCKET_PATH, () => {
  process.stdout.write(`browser daemon listening on ${SOCKET_PATH}\n`);
});

process.on("SIGTERM", async () => {
  shuttingDown = true;
  await resetContext();
  server.close();
  process.exit(0);
});

process.on("uncaughtException", async (err) => {
  process.stderr.write(`${err.message || String(err)}\n`);
  await resetContext();
});

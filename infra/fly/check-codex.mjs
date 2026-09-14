import assert from "node:assert/strict";
import { mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execFileSync, spawn } from "node:child_process";
import { createInterface } from "node:readline";

// Fresh disposable profile: never import the operator's ChatGPT session.
const home = mkdtempSync(join(tmpdir(), "elsewhere-codex-preflight-"));
const env = { ...process.env, CODEX_HOME: home };
delete env.OPENAI_API_KEY;
try {
  const schema = join(home, "schema");
  execFileSync("codex", ["app-server", "generate-json-schema", "--experimental", "--out", schema], { env, stdio: "ignore", timeout: 15000 });
  function containsDeviceLogin(dir) {
    return readdirSync(dir, { withFileTypes: true }).some((entry) => {
      const path = join(dir, entry.name);
      return entry.isDirectory() ? containsDeviceLogin(path)
        : entry.name.endsWith(".json") && readFileSync(path, "utf8").includes('"chatgptDeviceCode"');
    });
  }
  assert(containsDeviceLogin(schema), "Codex lacks the product's device authorization protocol");

  const child = spawn("codex", ["app-server", "--stdio"], { env, stdio: ["pipe", "pipe", "ignore"] });
  const lines = createInterface({ input: child.stdout });
  try {
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error("Codex protocol preflight timed out")), 15000);
      const finish = (error) => { clearTimeout(timer); error ? reject(error) : resolve(); };
      child.once("error", finish);
      child.once("exit", () => finish(new Error("Codex exited before account/read")));
      lines.on("line", (line) => {
        let reply;
        try { reply = JSON.parse(line); } catch { return; }
        if (reply.id === 1) {
          if (reply.error) return finish(new Error("Codex initialization rejected"));
          child.stdin.write(JSON.stringify({ method: "initialized" }) + "\n");
          child.stdin.write(JSON.stringify({ id: 2, method: "account/read", params: {} }) + "\n");
        } else if (reply.id === 2) {
          if (reply.error || reply.result?.account !== null) return finish(new Error("Fresh profile must be disconnected"));
          finish();
        }
      });
      child.stdin.write(JSON.stringify({ id: 1, method: "initialize", params: {
        clientInfo: { name: "elsewhere", version: "0.1.0" }, capabilities: { experimentalApi: true },
      } }) + "\n");
    });
    console.log("Linux Codex device-login schema and fresh-profile app-server handshake passed");
  } finally {
    lines.close();
    child.kill("SIGKILL");
    await new Promise((resolve) => {
      if (!child.pid || child.exitCode !== null || child.signalCode !== null) return resolve();
      child.once("exit", resolve);
    });
  }
} finally {
  rmSync(home, { recursive: true, force: true });
}

import net from "net";
import fs from "fs";
import { SOCKET_PATH } from "./browser-common.mjs";

function readRequest() {
  const fileFlag = process.argv.indexOf("--request");
  if (fileFlag !== -1 && process.argv[fileFlag + 1]) {
    return JSON.parse(fs.readFileSync(process.argv[fileFlag + 1], "utf8"));
  }
  if (process.argv[2] === "health") {
    return { action: "health" };
  }
  if (process.argv[2]) {
    return JSON.parse(process.argv[2]);
  }
  throw new Error("missing request JSON (--request path, health, or argv[2])");
}

function callDaemon(request, timeoutMs = 125_000) {
  return new Promise((resolve, reject) => {
    const socket = net.createConnection(SOCKET_PATH);
    let buffer = "";
    const timer = setTimeout(() => {
      socket.destroy();
      reject(new Error("browser daemon request timed out"));
    }, timeoutMs);

    socket.setEncoding("utf8");
    socket.on("error", (err) => {
      clearTimeout(timer);
      reject(err);
    });
    socket.on("data", (chunk) => {
      buffer += chunk;
      const newline = buffer.indexOf("\n");
      if (newline === -1) {
        return;
      }
      clearTimeout(timer);
      const line = buffer.slice(0, newline).trim();
      socket.end();
      try {
        resolve(JSON.parse(line));
      } catch (err) {
        reject(err);
      }
    });
    socket.on("connect", () => {
      socket.write(`${JSON.stringify(request)}\n`);
    });
  });
}

async function main() {
  const req = readRequest();
  const result = await callDaemon(req);
  if (!result.ok) {
    process.stderr.write(`${result.error || "browser daemon error"}\n`);
    process.exit(1);
  }
  process.stdout.write(`${JSON.stringify(result)}\n`);
}

main().catch((err) => {
  process.stderr.write(`${err.message || String(err)}\n`);
  process.exit(1);
});

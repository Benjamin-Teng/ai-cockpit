// 自動更新端到端真機驗證用的假更新伺服器（只聽 127.0.0.1:7790）。步驟見同目錄 auto-update-e2e.md。
// 宣稱最新正式版是 v0.1.2，下載時提供指定安裝檔的位元組（檔名改成 0.1.2）與對應的 SHA256SUMS.txt。
// 用法：node docs/research/2026-10-04/fake-update-server.js <安裝檔路徑>
// 每個請求記到 %TEMP%\cockpit-e2e\server.log（含 User-Agent，用來確認是 Cockpit 而不是瀏覽器發出的）。
const http = require("http");
const fs = require("fs");
const os = require("os");
const path = require("path");
const crypto = require("crypto");

const setupPath = process.argv[2];
if (!setupPath) {
  console.error("usage: node fake-update-server.js <path to ai-cockpit-<version>-x64-setup.exe>");
  process.exit(2);
}
const setup = fs.readFileSync(setupPath);
const name = "ai-cockpit-0.1.2-x64-setup.exe";
const sums = crypto.createHash("sha256").update(setup).digest("hex") + "  " + name + "\n";
const logDir = path.join(os.tmpdir(), "cockpit-e2e");
fs.mkdirSync(logDir, { recursive: true });
const logFile = path.join(logDir, "server.log");
const log = (m) => fs.appendFileSync(logFile, new Date().toISOString() + " " + m + "\n");

http
  .createServer((req, res) => {
    log(`${req.method} ${req.url} UA=${req.headers["user-agent"]}`);
    if (req.url === "/api/latest") {
      res.writeHead(200, { "Content-Type": "application/json" });
      return res.end(
        JSON.stringify({ tag_name: "v0.1.2", prerelease: false, assets: [{ name }, { name: "SHA256SUMS.txt" }] }),
      );
    }
    if (req.url === `/dl/v0.1.2/${name}`) {
      res.writeHead(200, { "Content-Length": setup.length });
      return res.end(setup);
    }
    if (req.url === "/dl/v0.1.2/SHA256SUMS.txt") {
      res.writeHead(200);
      return res.end(sums);
    }
    res.writeHead(404);
    res.end();
  })
  .listen(7790, "127.0.0.1", () => {
    log("listening; sums=" + sums.trim());
    console.log(`listening on http://127.0.0.1:7790 (log: ${logFile})`);
  });

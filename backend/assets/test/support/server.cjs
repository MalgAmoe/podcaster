const http = require("http");
const fs = require("fs");
const path = require("path");

const PORT = Number(process.env.PORT || 4173);
const staticRoot = path.resolve(__dirname, "../../../priv/static");

const mimeTypes = {
  ".css": "text/css; charset=utf-8",
  ".html": "text/html; charset=utf-8",
  ".js": "application/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".map": "application/json; charset=utf-8",
  ".png": "image/png",
  ".svg": "image/svg+xml",
  ".wav": "audio/wav",
};

function sendHtml(res) {
  const html = `<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <meta name="csrf-token" content="test-csrf-token" />
    <title>Poddyclip Test</title>
  </head>
  <body>
    <script>
      window.isGuest = true;
      window.userToken = null;
      window.userTotalSeconds = 0;
    </script>
    <div id="solid-process-app"></div>
    <script src="/assets/js/app.js"></script>
  </body>
</html>`;

  res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
  res.end(html);
}

function sendJson(res, body, status = 200) {
  res.writeHead(status, { "content-type": "application/json; charset=utf-8" });
  res.end(JSON.stringify(body));
}

function sendFile(res, filePath) {
  const ext = path.extname(filePath);
  const contentType = mimeTypes[ext] || "application/octet-stream";
  res.writeHead(200, { "content-type": contentType });
  fs.createReadStream(filePath).pipe(res);
}

function resolveStaticPath(urlPath) {
  const cleanPath = decodeURIComponent(urlPath.split("?")[0]);
  const resolvedPath = path.resolve(staticRoot, `.${cleanPath}`);
  if (!resolvedPath.startsWith(staticRoot)) {
    return null;
  }
  return resolvedPath;
}

const server = http.createServer((req, res) => {
  if (!req.url) {
    res.writeHead(400);
    res.end("Bad Request");
    return;
  }

  if (req.url === "/" || req.url === "/app" || req.url === "/app/") {
    sendHtml(res);
    return;
  }

  if (req.url === "/api/jobs/current") {
    sendJson(res, { job: null });
    return;
  }

  if (req.url.startsWith("/assets/") || req.url.startsWith("/images/")) {
    const filePath = resolveStaticPath(req.url);
    if (filePath && fs.existsSync(filePath) && fs.statSync(filePath).isFile()) {
      sendFile(res, filePath);
      return;
    }
  }

  res.writeHead(404, { "content-type": "text/plain; charset=utf-8" });
  res.end("Not Found");
});

server.listen(PORT, "127.0.0.1", () => {
  console.log(`Playwright test server listening on http://127.0.0.1:${PORT}`);
});

import logger from "./logger.class.js";
import http from "http";
import net from "net";
import url from "url";

class TestProxy {
  constructor({ mode = "log" } = {}) {
    this.port = 8888;
    this.host = "127.0.0.1";
    this.mode = mode;
    this.attempts = [];
    this.server = null;
  }

  async start() {
    logger.info(`Starting proxy server with mode ${this.mode}`);
    if (this.server) return;

    this.server = http.createServer((req, res) => {
      this.attempts.push({ type: "http", method: req.method, url: req.url });
      logger.info(`New HTTP connection attempt: ${req.url}`);

      if (this.mode === "block") {
        res.writeHead(503, { "Content-Type": "text/plain" });
        res.end("Blocked by test proxy");
        logger.debug("HTTP Connection blocked");
        return;
      }

      const hostHeader = req.headers["host"];
      let hostname, port;

      if (req.url.startsWith("http://") || req.url.startsWith("https://")) {
        const parsedUrl = url.parse(req.url);
        hostname = parsedUrl.hostname;
        port = parsedUrl.port || (parsedUrl.protocol === "https:" ? 443 : 80);
        req.url = parsedUrl.path;
      } else {
        [hostname, port] = hostHeader.split(":");
        port = port || 80;
      }

      const options = {
        hostname,
        port,
        path: req.url,
        method: req.method,
        headers: req.headers,
      };

      const proxyReq = http.request(options, (proxyRes) => {
        res.writeHead(proxyRes.statusCode, proxyRes.headers);
        proxyRes.pipe(res);
      });

      req.pipe(proxyReq);

      proxyReq.on("error", (err) => {
        res.writeHead(500);
        res.end("Proxy error: " + err.message);
      });
    });

    // Handle HTTPS CONNECT
    this.server.on("connect", (req, clientSocket, head) => {
      this.attempts.push({ type: "https", host: req.url });
      logger.info(`New HTTPS connection attempt: ${req.url}`);

      if (this.mode === "block") {
        logger.debug("HTTPS Connection blocked");
        clientSocket.write("HTTP/1.1 503 Service Unavailable\r\n\r\n");
        clientSocket.end();
        return;
      }

      const [host, port] = req.url.split(":");
      const serverSocket = net.connect(port || 443, host, () => {
        clientSocket.write("HTTP/1.1 200 Connection Established\r\n\r\n");
        serverSocket.write(head);
        serverSocket.pipe(clientSocket);
        clientSocket.pipe(serverSocket);
      });

      serverSocket.on("error", () => {
        clientSocket.write("HTTP/1.1 500 Connection error\r\n\r\n");
        clientSocket.end();
      });
    });

    return new Promise((resolve) => {
      this.server.listen(this.port, this.host, resolve);
    });
  }

  async stop() {
    logger.info("attempts:", this.attempts);
    if (!this.server) return;
    return new Promise((resolve) => {
      this.server.close(() => {
        this.server = null;
        logger.info("Proxy server stopped");
        resolve();
      });
    });
  }
}

export default TestProxy;

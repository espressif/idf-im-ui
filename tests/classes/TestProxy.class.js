import logger from "./logger.class.js";
import http from "http";
import net from "net";
import url from "url";

class TestProxy {
  constructor({ mode = "log", blockedDomains = [] } = {}) {
    this.port = 8888;
    this.host = "127.0.0.1";
    this.mode = mode; // "log", "block" or "block-list"
    this.attempts = [];
    this.server = null;
    this.blockedDomains = blockedDomains.map((domain) => domain.toLowerCase());
  }

  matchesBlocked(hostname) {
    if (!hostname) return false;
    const host = hostname.toLowerCase();
    return this.blockedDomains.some(
      (d) => host === d || host.endsWith("." + d)
    );
  }

  setEnvironment() {
    logger.info("Setting proxy environment variables");
    const proxyUrl = "http://127.0.0.1:8888";
    let environment = { ...process.env };
    environment.HTTP_PROXY = proxyUrl;
    environment.HTTPS_PROXY = proxyUrl;
    environment.http_proxy = proxyUrl;
    environment.https_proxy = proxyUrl;
    environment.FTP_PROXY = proxyUrl;
    environment.ftp_proxy = proxyUrl;
    environment.NO_PROXY = "127.0.0.1,localhost,::1";
    environment.no_proxy = "127.0.0.1,localhost,::1";
    environment.CARGO_HTTP_PROXY = proxyUrl;
    environment.CARGO_HTTPS_PROXY = proxyUrl;
    environment.GIT_PROXY_COMMAND = "";
    environment.npm_config_proxy = proxyUrl;
    environment.npm_config_https_proxy = proxyUrl;
    environment.PIP_PROXY = proxyUrl;
    process.env = { ...environment };
  }

  async start() {
    if (this.server) {
      logger.info("Proxy server already running");
      return;
    }
    logger.info(`Starting proxy server with mode ${this.mode}`);
    this.setEnvironment();
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

      if (this.mode === "block-list" && this.matchesBlocked(hostname)) {
        this.attempts.push({
          type: "http",
          method: req.method,
          url: req.url,
          blocked: true,
        });
        res.writeHead(503, { "Content-Type": "text/plain" });
        res.end("Blocked by test proxy (domain block-list)");
        logger.debug(`HTTP Connection to ${hostname} blocked by domain list`);
        return;
      }

      const options = {
        hostname,
        port,
        path: req.url,
        method: req.method,
        headers: req.headers,
      };

      logger.info(`Proxying request to ${hostname}:${port}${req.url}`);
      logger.info(options);

      const proxyReq = http.request(options, (proxyRes) => {
        res.writeHead(proxyRes.statusCode, proxyRes.headers);
        proxyRes.pipe(res);
      });

      req.pipe(proxyReq);

      proxyReq.on("error", (err) => {
        logger.info("HTTP Proxy request error:", err);
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

      if (this.mode === "block-list" && this.matchesBlocked(host)) {
        this.attempts.push({ type: "https", host, blocked: true });
        logger.info(`HTTPS Connection to ${host} blocked by domain list`);
        clientSocket.write("HTTP/1.1 503 Service Unavailable\r\n\r\n");
        clientSocket.end();
        return;
      }
      logger.info(`Proxying CONNECT to ${host}:${port}`);

      const serverSocket = net.connect(port || 443, host, () => {
        clientSocket.write("HTTP/1.1 200 Connection Established\r\n\r\n");
        serverSocket.write(head);
        serverSocket.pipe(clientSocket);
        clientSocket.pipe(serverSocket);
      });

      serverSocket.on("error", (err) => {
        logger.info("HTTPS Connection error:", err);
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

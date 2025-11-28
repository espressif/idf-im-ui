import pty from "node-pty";
import os from "os";
import logger from "./logger.class.js";
import stripAnsi from "strip-ansi";

class CLITestRunner {
  constructor() {
    this.process = null;
    this.output = "";
    this.exited = false;
    this.exitCode = null;
    this.error = null;
    this.lastDataTimestamp = Date.now();
    this.prompt = os.platform() !== "win32" ? "$" : ">";
    this.command = os.platform() !== "win32" ? "bash" : "powershell.exe";
    this.args =
      os.platform() !== "win32"
        ? []
        : ["-ExecutionPolicy", "Bypass", "-NoProfile"];
  }

  async runIDFTerminal(loadScript, timeout = 3000) {
    try {
      await this.start();
      const loadCommand =
        os.platform() !== "win32"
          ? `source ${loadScript}`
          : `. "${loadScript}"`;
      logger.debug(`Script load command sent to terminal ${loadCommand}`);
      this.sendInput(`${loadCommand}`);
      const startTime = Date.now();
      while (Date.now() - startTime < timeout) {
        if (!this.exited && !this.error && this.output.includes("(venv)")) {
          return Promise.resolve();
        }
        await new Promise((resolve) => setTimeout(resolve, 200));
      }
      logger.info("Failed to load IDF terminal within timeout");
      return Promise.reject();
    } catch {
      logger.debug("Error loading IDF terminal");
      return Promise.reject();
    }
  }

  async start({ command = this.command, fullArgs = this.args } = {}) {
    logger.debug(
      `Starting terminal emulator ${this.command} with args ${this.args}`
    );

    this.process = pty.spawn(command, fullArgs, {
      name: "eim-terminal",
      cols: 80,
      rows: 30,
      cwd: process.cwd(),
      env: process.env,
    });
    this.exited = false;

    this.process.onData((data) => {
      let cleanData = stripAnsi(data);
      cleanData = cleanData.replace(/\\[\r\n]+/g, "");
      cleanData = cleanData.replace(/[\r\n]+/g, "");
      this.output += cleanData;
      this.lastDataTimestamp = Date.now();
    });

    this.process.onExit(({ exitCode }) => {
      this.exited = true;
      this.exitCode = exitCode;
      logger.debug(`Terminal exited with code:>>${exitCode}<<`);
    });

    this.process.on("error", (error) => {
      this.error = error;
      this.exited = true;
      logger.debug(`Terminal error:>>${error}<<`);
    });

    await new Promise((resolve) => {
      setTimeout(resolve, 2000);
    });

    // Wait until prompt is ready
    if (!this.exited && !this.error) {
      try {
        if (await this.waitForPrompt()) {
          return Promise.resolve();
        } else {
          logger.info(`No prompt detected >>${this.output}<<< `);
          Promise.reject("Timeout without a prompt");
        }
      } catch (error) {
        logger.info(`Error detecting prompt >>${this.output}<<< `);
        return Promise.reject(error);
      }
    } else {
      return Promise.reject(`Could not start terminal`);
    }
  }

  sendInput(input) {
    logger.debug(`Attempting to send ${input.replace(/\r$/, "")} to terminal`);
    if (this.process && !this.exited) {
      try {
        const lineEnding = os.platform() !== "win32" ? "\n" : "\r";
        this.process.write(`${input}${lineEnding}`);
      } catch (error) {
        logger.info(`Error sending input:>>${error}<<`);
        this.error = error;
        this.exited = true;
      }
    } else {
      logger.info("Attempted to send input, but process is not running");
    }
  }

  async waitForOutput(expectedOutput, timeout = 10000) {
    const startTime = Date.now();
    while (Date.now() - startTime < timeout) {
      if (this.output.includes(expectedOutput)) {
        return true;
      }
      if (this.exited) {
        return false;
      }
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    return false;
  }

  async waitForPrompt(timeout = 3000) {
    const startTime = Date.now();
    while (Date.now() - startTime < timeout) {
      if (this.output.slice(-10).includes(this.prompt)) {
        return true;
      }
      await new Promise((resolve) => setTimeout(resolve, 200));
    }
    return false;
  }

  async stop(timeout = 3000) {
    if (this.process && !this.exited) {
      try {
        this.sendInput("exit");
        const exitTime = Date.now();
        while (Date.now() - exitTime < timeout) {
          if (this.exited) {
            logger.info("terminal exited gracefully");
            return Promise.resolve();
          }
          await new Promise((resolve) => setTimeout(resolve, 200));
        }
        logger.info("Terminal didn't exit gracefully, repeat Attempt");
        this.process.write("\x03");
        this.process.write("\x03");
        this.sendInput("exit");
        const closeTime = Date.now();
        while (Date.now() - closeTime < timeout) {
          if (this.exited) {
            logger.info("terminal exited gracefully");
            return Promise.resolve();
          }
          await new Promise((resolve) => setTimeout(resolve, 200));
        }
        logger.info(
          "Terminal didn't exit gracefully, abandoning task, should be terminated by node."
        );
        throw new Error("Could not stop terminal gracefully");
      } catch (error) {
        this.exited = true;
        this.process = null;
        throw error;
      }
    } else {
      logger.debug("Terminal has already exited");
      this.process = null;
      this.exited = true;
      this.output = "";
      return Promise.resolve();
    }
  }
}

export default CLITestRunner;

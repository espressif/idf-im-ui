
/**
 * This class is used to run a terminal emulator for the CLI tests.
 *
 * The terminal emulation is done using node-pty.
 * Several methods are provided to allow better control of the input and output of the terminal process.
 *
 *
 * The terminal process is started and keep running until the stop process is called, or any error occurs.
 */
import pty from "node-pty";
import os from "os";
import logger from "./logger.class.js";
import stripAnsi from "strip-ansi";
import { spawn } from "child_process";

class CLITestRunner {
  constructor() {
    this.process = null;
    this.output = "";
    this.exited = false;
    this.exitCode = null;
    this.error = null;
    this.lastDataTimestamp = Date.now();
    this.prompt = os.platform() !== "win32" ? ["$", "#"] : [">"];
    this.command = os.platform() !== "win32" ? "bash" : "powershell.exe";
    this.args =
      os.platform() !== "win32"
        ? []
        : ["-ExecutionPolicy", "Bypass", "-NoProfile"];
  }

  // Function to start a terminal instance and load EIM activation script, the script path should be provided as argument
  async runIDFTerminal(loadScript, timeout = 5000) {
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

  // Run a single command in a fresh subshell, capture stdout and exit code.
  // Used by verification tests that need to invoke (not source) a generated
  // script and observe its output, e.g. the activation script's `-e` flag.
  //
  // On POSIX the command is run through the interactive bash pty: the
  // command is followed by an exit-code probe (`echo "EXIT=$?"`) and a
  // unique sentinel; both are used to detect completion and recover the
  // exit code from the captured stream.
  //
  // On Windows the interactive PowerShell pty is unreliable for this:
  // PSReadLine echoes typed commands (so the sentinel and the `EXIT=`
  // probe text both appear in the buffer before the command runs) and
  // shows history autosuggest from earlier tests, which pollutes the
  // capture. We therefore spawn a fresh non-interactive PowerShell
  // subprocess and capture stdout/stderr + exit code directly. The
  // `sentinel` parameter is unused on the Windows path.
  //
  // Returns `{ output, exitCode }` where `output` is the captured buffer
  // (also left on `this.output` for debugging) and `exitCode` is the
  // integer the previous command exited with, or `null` if the sentinel
  // never appeared / the subprocess was killed.
  async runAndCapture(command, sentinel, timeout = 30000) {
    if (os.platform() === "win32") {
      logger.debug(
        `runAndCapture (subprocess): powershell.exe -Command ${command}`
      );
      return new Promise((resolve) => {
        let output = "";
        let timer = null;
        const child = spawn(
          "powershell.exe",
          ["-ExecutionPolicy", "Bypass", "-NoProfile", "-Command", command]
        );
        child.stdout.on("data", (data) => {
          output += data.toString();
        });
        child.stderr.on("data", (data) => {
          output += data.toString();
        });
        child.on("close", (exitCode) => {
          if (timer) clearTimeout(timer);
          this.output = output;
          resolve({ output, exitCode });
        });
        child.on("error", (error) => {
          if (timer) clearTimeout(timer);
          logger.info(`runAndCapture subprocess error: ${error}`);
          this.output = output;
          resolve({ output, exitCode: null });
        });
        timer = setTimeout(() => {
          try {
            child.kill();
          } catch (e) {
            logger.info(`Failed to kill runAndCapture subprocess: ${e}`);
          }
        }, timeout);
      });
    }

    // Only spawn a fresh pty if there isn't one yet. If a test set
    // up state with sendInput() (e.g. sourcing an activate script)
    // and then calls runAndCapture() to query that state, starting a
    // new bash would discard the sourced env vars. Reusing the
    // existing pty preserves the state.
    if (!this.process || this.exited) {
      await this.start();
    }
    // Drop any pre-existing output (start, prompt, etc.) so the returned
    // buffer only contains what the command itself produced.
    this.output = "";

    const exitProbe = `echo "EXIT=$?"`;

    this.sendInput(`${command} ; ${exitProbe} ; echo "${sentinel}"`);

    const startTime = Date.now();
    let exitCode = null;
    while (Date.now() - startTime < timeout) {
      if (this.output.includes(sentinel)) {
        const match = this.output.match(/EXIT=(\d+)/);
        if (match) exitCode = parseInt(match[1], 10);
        break;
      }
      if (this.exited) break;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }

    // Trim the trailing probe + sentinel from the captured buffer so the
    // caller sees only the command's own output.
    const sentinelIdx = this.output.lastIndexOf(sentinel);
    const captured =
      sentinelIdx > 0 ? this.output.slice(0, sentinelIdx) : this.output;
    return { output: captured, exitCode };
  }

  // Function to start a terminal instance, The process will be kept running in the background.
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

    // Stream function to capture the terminal output
    // Note the variable lastDataTimestamp is updated to the current time to allow checking for idle terminal
    this.process.onData((data) => {
      let cleanData = stripAnsi(data);
      cleanData = cleanData.replace(/\\[\r\n]+/g, "");
      cleanData = cleanData.replace(/[\r\n]+/g, "");
      this.output += cleanData;
      this.lastDataTimestamp = Date.now();
      if (process.env.SHOW_RUNNER_OUTPUT === "1") {
        process.stdout.write(data);
      }
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

  // method to call EIM binary on specific path with the specified arguments
  callEIM(eimCliPath, args = []) {
    const fullArgs = ["--do-not-track", "true", ...args];
    const argLine = fullArgs.join(" ");
    logger.debug(
      `Calling EIM from path ${eimCliPath} with arguments ${argLine}`
    );
    // PowerShell parses "path" --flag as an error; use the call operator &.
    // Bash treats a leading & as background — only use & on Windows.
    if (os.platform() === "win32") {
      this.sendInput(`& "${eimCliPath}" ${argLine}`);
    } else {
      const quoted = /\s/.test(eimCliPath)
        ? `"${eimCliPath.replace(/"/g, '\\"')}"`
        : eimCliPath;
      this.sendInput(`${quoted} ${argLine}`);
    }
  }

  // method to send a string to the terminal, return character is added to any string provided
  // This method should not be use to call EIM binary, use callEIM method instead.
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

  // method to wait for a specific output to be present in the terminal output, timeout is set to 10 seconds by default
  // One strategy is to send the command as `command ; echo "output" ; echo "done"` and then wait for "outputdone" to be printed in the terminal
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

  // method to wait for the terminal prompt to be present in the terminal output, timeout is set to 3 seconds by default
  // Although this method may be brittle, it works for simple outputs printed in the terminal, not recommended for complex outputs like project builds
  async waitForPrompt(timeout = 3000) {
    const startTime = Date.now();
    while (Date.now() - startTime < timeout) {
      if (this.prompt.some(prompt => this.output.slice(-10).includes(prompt))) {
        return true;
      }
      await new Promise((resolve) => setTimeout(resolve, 200));
    }
    return false;
  }

  // method to stop the terminal process, timeout is set to 3 seconds by default
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

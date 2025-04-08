import pty from "node-pty";
import os from "os";
import logger from "./logger.class.js";

class CLITestRunner {
    constructor() {
        this.process = null;
        this.output = "";
        this.exited = false;
        this.exitCode = null;
        this.error = null;
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
            this.sendInput(`${loadCommand}\r`);
            const startTime = Date.now();
            while (Date.now() - startTime < timeout) {
                if (
                    !this.exited &&
                    !this.error &&
                    this.output.includes("(python)")
                ) {
                    return Promise.resolve();
                }
                await new Promise((resolve) => setTimeout(resolve, 200));
            }
            logger.info("Failed to terminate terminal process");
            return Promise.resolve();
        } catch {
            logger.debug("Error loading IDF terminal");
            return Promise.resolve();
        }
    }

    async start(command = this.command, fullArgs = this.args, timeout = 5000) {
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
            logger.debug(data);
            this.output += data;
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
                await this.waitForPrompt();
                return Promise.resolve();
            } catch (error) {
                logger.info(`Error detecting prompt >>${this.output}<<< `);
                return Promise.reject(error);
            }
        } else {
            return Promise.reject(`Could not start terminal`);
        }
    }

    sendInput(input) {
        logger.debug(
            `Attempting to send ${input.replace(/\r$/, "")} to terminal`
        );
        if (this.process && !this.exited) {
            try {
                this.process.write(input);
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
            if (this.output.slice(-20).includes(this.prompt)) {
                return Promise.resolve();
            }
            await new Promise((resolve) => setTimeout(resolve, 200));
        }
        return Promise.reject("Timeout without a prompt");
    }

    async stop(timeout = 3000) {
        if (this.process && !this.exited) {
            try {
                this.sendInput("exit\r");
                const exitTime = Date.now();
                while (Date.now() - exitTime < timeout) {
                    if (this.exited) {
                        logger.info("terminal exited gracefully");
                        return Promise.resolve();
                    }
                    await new Promise((resolve) => setTimeout(resolve, 200));
                }
                logger.info("Terminal didn't exit gracefully, repeat Attempt");
                this.sendInput("\x03");
                this.sendInput("\x03");
                this.sendInput("exit\r");
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

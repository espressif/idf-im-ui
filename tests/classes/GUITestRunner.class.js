/**
 * This class is used to run a GUI application for the GUI tests.
 * The GUI application is launched using selenium webdriver tauri-driver.
 * Several methods are provided to allow better control of the GUI application.
 * Alternatively pure selenium webdriver commands can be used to control the GUI application.
 *
 * The GUI application is started and keep running until the stop process is called, or any error occurs.
 *
 */


import os from "os";
import path from "path";
import fs from "fs";
import logger from "./logger.class.js";
import { spawn } from "child_process";
import { Builder, By, Capabilities, until } from "selenium-webdriver";

class GUITestRunner {
  constructor(application, args = []) {
    args = ["--do-not-track=true", ...args];
    logger.debug(`Starting EIM from path ${application} with arguments ${args}`);

    this.application = application;
    this.capabilities = new Capabilities();

    this.capabilities.set("tauri:options", {
      application,
      args,
    });
    this.capabilities.setBrowserName("wry");
  }

  // Function to launch the GUI application
  // Wrapped in a hard outer deadline: `_start()` below has async steps
  // (notably `Builder().build()`, which creates the WebDriver session) that
  // carry no timeout of their own. If one of those hangs - e.g. tauri-driver
  // becomes ready and launches the app, but the session-creation handshake
  // itself stalls - nothing inside `_start()` ever rejects, so the retry
  // loop's cleanup never runs and mocha's own hook timeout (60s) is left to
  // kill the test, leaving tauri-driver/the app running with no cleanup at
  // all. Racing against a real timer here guarantees `stop()` always runs
  // and that we always throw ourselves well before that hook timeout.
  async start() {
    let deadlineTimer;
    const deadlineMs = 45000;
    const deadline = new Promise((_, reject) => {
      deadlineTimer = setTimeout(() => {
        reject(
          new Error(`GUI application did not become ready within ${deadlineMs}ms`)
        );
      }, deadlineMs);
    });
    const startPromise = this._start();
    // If the deadline wins the race, `startPromise` is still running in the
    // background (Promise.race doesn't cancel it) and will settle later on
    // its own, e.g. once `stop()` below tears down tauri-driver out from
    // under it. Nothing awaits that outcome, so give it a no-op handler to
    // avoid an unhandled rejection.
    startPromise.catch(() => {});
    try {
      await Promise.race([startPromise, deadline]);
    } catch (error) {
      await this.stop();
      throw error;
    } finally {
      clearTimeout(deadlineTimer);
    }
  }

  async _start() {
    logger.info("Lauching Tauri Driver");
    const tauriDriverPath = path.resolve(
      os.homedir(),
      ".cargo",
      "bin",
      os.platform() === "win32" ? "tauri-driver.exe" : "tauri-driver"
    );
    try {
      this.tauriDriver = spawn(tauriDriverPath, [], {
        stdio: ["ignore", "pipe", "pipe"],
      });
      // tauri-driver's own stdout/stderr carry the actual reason a session
      // fails to be created (e.g. the underlying msedgedriver/webview error
      // behind "DevToolsActivePort file doesn't exist"). Piping straight to
      // process.stdout/stderr used to send this into the raw CI console,
      // which isn't captured by any uploaded artifact - every session
      // failure was undiagnosable after the fact. Log it to a file instead
      // (tests/*.log is already picked up by the CI artifact upload).
      const tauriDriverLogPath = path.join(
        import.meta.dirname,
        "..",
        "tauri-driver-output.log"
      );
      const logLine = (chunk) =>
        fs.appendFileSync(tauriDriverLogPath, chunk.toString());
      logLine(`\n--- tauri-driver launched at ${new Date().toISOString()} (pid ${this.tauriDriver.pid}) ---\n`);
      this.tauriDriver.stdout.on("data", logLine);
      this.tauriDriver.stderr.on("data", logLine);
    } catch (error) {
      logger.info("Error launching Tauri driver:", error);
      throw error;
    }

    const tauriReady = await this.waitForTauriDriver(10000);
    if (!tauriReady) {
      throw new Error(
        `tauri-driver did not become ready in time (path: ${tauriDriverPath})`
      );
    }

    const maxSessionAttempts = 3;
    for (let attempt = 1; attempt <= maxSessionAttempts; attempt++) {
      try {
        this.driver = await new Builder()
          .withCapabilities(this.capabilities)
          .usingServer("http://127.0.0.1:4444")
          .build();
        return;
      } catch (error) {
        logger.info(
          `Error building driver (attempt ${attempt}/${maxSessionAttempts}):`,
          error
        );
        if (attempt === maxSessionAttempts) {
          throw error;
        }
        await new Promise((resolve) => setTimeout(resolve, 2000 * attempt));
      }
    }
  }

  async waitForTauriDriver(timeout = 10000) {
    const end = Date.now() + timeout;
    while (Date.now() < end) {
      try {
        const controller = new AbortController();
        const abortTimer = setTimeout(() => controller.abort(), 2000);
        try {
          const response = await fetch("http://127.0.0.1:4444/status", {
            signal: controller.signal,
          });
          if (response.ok) {
            return true;
          }
        } finally {
          clearTimeout(abortTimer);
        }
      } catch (_error) {
        // tauri-driver not ready yet
      }
      await new Promise((resolve) => setTimeout(resolve, 300));
    }
    return false;
  }

  // Function to stop the GUI application
  async stop() {
    if (this.driver) {
      try {
        await this.driver.quit();
      } catch (error) {
        logger.info("Error quitting driver:", error);
      }
    }
    try {
      if (this.tauriDriver && this.tauriDriver.pid) {
        if (os.platform() === "win32") {
          await new Promise((resolve) => {
            const killer = spawn("taskkill", [
              "/pid",
              String(this.tauriDriver.pid),
              "/T",
              "/F",
            ]);
            killer.on("close", resolve);
            killer.on("error", resolve);
          });
        } else {
          this.tauriDriver.kill();
        }
      }
    } catch (error) {
      logger.info("Error closing Tauri driver:", error);
    }
  }

  // method to find an element by its HTML id attribute
  async findById(id, timeout = 5000) {
    try {
      const element = await this.driver.wait(
        until.elementLocated(By.id(id)),
        timeout,
        `Element with id ${id} not found`
      );
      logger.debug(`Selected html element ${await element.getTagName()}`);
      return element;
    } catch (error) {
      logger.debug(`Error during selection: ${error}`);
      return false;
    }
  }

  // method to find an element by its HTML class attribute
  // if more than one element exists it will only capture the first one
  async findByClass(className, timeout = 5000) {
    try {
      const element = await this.driver.wait(
        until.elementLocated(By.className(className)),
        timeout,
        `Element with class ${className} not found`
      );
      logger.debug(`Selected html element ${await element.getTagName()}`);
      return element;
    } catch (error) {
      logger.debug(`Error during selection: ${error}`);
      return false;
    }
  }

  // method to find multiple elements by their HTML class attribute
  // If only one element exists it will still return an array with a single element
  async findMultipleByClass(className, timeout = 5000) {
    try {
      const elements = await this.driver.wait(
        until.elementsLocated(By.className(className)),
        timeout,
        `Elements with class ${className} not found`
      );
      logger.debug(`Selected html elements matching class ${className}`);
      return elements;
    } catch (error) {
      logger.debug(`Error during selection: ${error}`);
      return false;
    }
  }

  // method to find an element by its HTML CSS attribute
  async findByCSS(cssAttribute, timeout = 5000) {
    try {
      const element = await this.driver.wait(
        until.elementLocated(By.css(cssAttribute)),
        timeout,
        `Element with attribute ${cssAttribute} not found`
      );
      logger.debug(`Selected html element ${await element.getTagName()}`);
      return element;
    } catch (error) {
      logger.debug(`Error during selection: ${error}`);
      return false;
    }
  }

  // method to find an element by its HTML data-id attribute
  // Data-id names were conveniently added to several objects to allow easier identification in the GUI tests
  async findByDataId(dataId, timeout = 5000) {
    try {
      const element = await this.driver.wait(
        until.elementLocated(By.css(`[data-id="${dataId}"]`)),
        timeout,
        `Element with test ID ${dataId} not found`
      );
      logger.debug(`Selected html element ${await element.getTagName()}`);
      return element;
    } catch (error) {
      logger.debug(`Error during selection: ${error}`);
      return false;
    }
  }

  // method to find an element by its text content
  async findByText(text, timeout = 5000) {
    try {
      const element = await this.driver.wait(
        until.elementLocated(By.xpath(`//*[contains(text(), '${text}')]`)),
        timeout,
        `Element containing text "${text}" not found`
      );
      logger.debug(`Selected html element ${await element.getTagName()}`);
      return element;
    } catch (error) {
      logger.debug(`Error during selection: ${error}`);
      return false;
    }
  }

  // method to find an element by its relation to another element
  // Use this method to select an element upstream or downstream to a known element. Relation and tag refers to the element you want to find.
  // text is the text content of the reference element.
  async findByRelation(relation, tag, text, timeout = 5000) {
    try {
      const element = await this.driver.wait(
        until.elementLocated(
          By.xpath(`//*[contains(text(), '${text}')]/${relation}::${tag}`)
        ),
        timeout,
        `Element ${tag} containing text "${text}" not found`
      );
      logger.debug(`Selected html element ${await element.getTagName()}`);
      return element;
    } catch (error) {
      logger.debug(`Error during selection: ${error}`);
      return false;
    }
  }

  // method to click a button by its text content
  async clickButton(text, timeout = 5000) {
    try {
      const button = await this.driver.wait(
        until.elementLocated(
          By.xpath(`//*[contains(text(), '${text}')]/ancestor-or-self::button`)
        ),
        timeout,
        `Button with text "${text}" not found`
      );
      logger.debug(
        `Selected button element with text ${await button.getText()}`
      );
      await this.driver.executeScript("arguments[0].click();", button);
      return true;
    } catch (error) {
      logger.debug(`Error during selection: ${error}`);
      return false;
    }
  }

  // method to click any element by its text content
  async clickElement(text, timeout = 5000) {
    try {
      const element = await this.driver.wait(
        until.elementLocated(By.xpath(`//*[contains(text(), '${text}')]`)),
        timeout,
        `Element with text "${text}" not found`
      );
      logger.debug(
        `Selected element ${await element.getTagName()} with text ${await element.getText()}`
      );
      await this.driver.executeScript("arguments[0].click();", element);
      return true;
    } catch (error) {
      logger.debug(`Error during selection: ${error}`);
      return false;
    }
  }

  // Method to take a screenshot of the current GUI state
  // Thsi is mostly used for debug
  async takeScreenshot(filename) {
    try {
      const screenshot = await this.driver.takeScreenshot();
      fs.writeFileSync(filename, screenshot, "base64");
    } catch (error) {
      logger.info("Error taking screenshot:", error);
      throw error;
    }
  }
}

export default GUITestRunner;

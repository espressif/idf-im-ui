/**
 * Shared mocha hooks and terminal helpers for the CLI test scripts.
 * Hooks must be registered from inside a `describe` callback.
 */

import { beforeEach, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";

async function startTerminal(testRunner) {
  try {
    await testRunner.start();
  } catch (error) {
    logger.info("Error to start terminal");
    logger.debug(`Error: ${error}`);
  }
}

async function stopTerminal(testRunner) {
  try {
    await testRunner.stop();
  } catch (error) {
    logger.info("Error to clean up terminal after test");
    logger.info(` Error: ${error}`);
  }
}

function logFailedTest(currentTest, testRunner, { tail = 2000, full = true } = {}) {
  logger.info(`Test failed: ${currentTest.title}`);
  if (testRunner) {
    logger.info(`Terminal output: >>\r ${testRunner.output.slice(-tail)}`);
    full && logger.debug(`Terminal output on failure: >>\r ${testRunner.output}`);
  }
}

// Steps that each open their own terminal: once a step fails the remaining
// steps are skipped, and any terminal left open by a step is stopped.
function registerSequentialStepHooks({
  getRunner,
  clearRunner,
  skipMessage = "Test failed, skipping next tests",
}) {
  let stepFailed = false;

  beforeEach(async function () {
    this.timeout(10000);
    if (stepFailed) {
      logger.info(skipMessage);
      this.skip();
    }
  });

  afterEach(async function () {
    this.timeout(20000);
    const testRunner = getRunner();
    if (this.currentTest.state === "failed") {
      logFailedTest(this.currentTest, testRunner);
      stepFailed = true;
    }
    if (testRunner) {
      await stopTerminal(testRunner);
      clearRunner();
    }
  });
}

// Each step gets a fresh terminal running `eim wizard`
function registerWizardTerminalHooks({ pathToEIM, setRunner, getRunner, startTimeout }) {
  beforeEach(async function () {
    this.timeout(startTimeout);
    const testRunner = new CLITestRunner();
    setRunner(testRunner);
    try {
      await testRunner.start();
      testRunner.callEIM(pathToEIM, ["wizard"]);
    } catch (error) {
      logger.info(`Error starting process: ${error}`);
      logger.debug(` Error: ${error}`);
    }
  });

  afterEach(async function () {
    this.timeout(20000);
    const testRunner = getRunner();
    if (this.currentTest.state === "failed") {
      logFailedTest(this.currentTest, testRunner, { tail: 1000 });
    }
    if (testRunner) {
      await stopTerminal(testRunner);
      setRunner(null);
    }
  });
}

// Poll the terminal until `text` is printed, the terminal goes idle, the app
// panics (when `detectPanic`) or `timeoutMs` elapses. Returns the start time.
async function waitForTerminalOutput(
  testRunner,
  text,
  {
    timeoutMs = 3600000,
    idleMs = 600000,
    pollMs = 1000,
    detectPanic = true,
    successMessage = ">>>>>>>Completed!!!",
    timeoutMessage = "Installation timed out after 1 hour",
  } = {}
) {
  const startTime = Date.now();
  while (Date.now() - startTime < timeoutMs) {
    if (Date.now() - testRunner.lastDataTimestamp >= idleMs) {
      logger.info(">>>>>>>Exited due to Idle terminal!!!!!");
      break;
    }
    if (detectPanic && (await testRunner.waitForOutput("panicked", 1000))) {
      logger.info(">>>>>>>Rust App failure!!!!");
      break;
    }
    if (await testRunner.waitForOutput(text, 1000)) {
      logger.info(successMessage);
      break;
    }
    await new Promise((resolve) => setTimeout(resolve, pollMs));
  }
  if (timeoutMessage && Date.now() - startTime >= timeoutMs) {
    logger.info(timeoutMessage);
  }
  return startTime;
}

export {
  startTerminal,
  stopTerminal,
  logFailedTest,
  registerSequentialStepHooks,
  registerWizardTerminalHooks,
  waitForTerminalOutput,
};

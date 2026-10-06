/**
 * Shared mocha hooks and steps for the GUI test scripts.
 * Hooks must be registered from inside a `describe` callback.
 */

import { expect } from "chai";
import { before, after, beforeEach, afterEach } from "mocha";
import GUITestRunner from "../classes/GUITestRunner.class.js";
import logger from "../classes/logger.class.js";
import { tGui } from "./i18n.js";

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function startGUIApp(eimRunner) {
  try {
    await eimRunner.start();
  } catch (err) {
    logger.info("Error starting EIM application");
    throw err;
  }
}

// Returns true when the application was stopped
async function stopGUIApp(eimRunner) {
  try {
    await eimRunner.stop();
    return true;
  } catch (error) {
    logger.info("Error to close EIM application");
    return false;
  }
}

// Start the EIM application GUI before the tests and stop it after them.
// `afterStart` runs at the end of the `before` hook with the started runner.
function registerGUIAppLifecycle({
  pathToEIM,
  getRunner,
  setRunner,
  startTimeout = 60000,
  afterStart,
}) {
  before(async function () {
    this.timeout(startTimeout);
    const eimRunner = new GUITestRunner(pathToEIM);
    setRunner(eimRunner);
    await startGUIApp(eimRunner);
    if (afterStart) await afterStart(eimRunner);
  });

  after(async function () {
    this.timeout(5000);
    if (await stopGUIApp(getRunner())) setRunner(null);
  });
}

// Save a screenshot when a test fails and, with `skipAfterFailure`, skip the
// tests that follow a failed one.
function registerGUIFailureHooks({
  id,
  getRunner,
  skipAfterFailure = true,
  skipTimeout,
  screenshotTimeout,
}) {
  let stepFailed = false;

  if (skipAfterFailure) {
    beforeEach(async function () {
      if (skipTimeout) this.timeout(skipTimeout);
      if (stepFailed) {
        logger.info("Test failed, skipping next tests");
        this.skip();
      }
    });
  }

  afterEach(async function () {
    if (screenshotTimeout) this.timeout(screenshotTimeout);
    const eimRunner = getRunner();
    if (this.currentTest.state === "failed" && eimRunner?.driver) {
      await eimRunner.takeScreenshot(`${id} ${this.currentTest.title}.png`);
      logger.info(`Screenshot saved as ${id} ${this.currentTest.title}.png`);
    }
    if (this.currentTest.state === "failed") stepFailed = true;
  });
}

async function expectWelcomePage(eimRunner, initialWaitMs) {
  // Wait for the header to be present
  await sleep(initialWaitMs);
  const header = await eimRunner.findByDataId("welcome-header", 25000);
  expect(header, "Expected welcome header").to.not.be.false;
  const text = await header.getText();
  expect(text, "Expected welcome text").to.equal(
    `${tGui("welcome.welcome")} ESP-IDF ${tGui("welcome.title")}`
  );
}

// Checks the manage-installations card, opens the dashboard and returns the
// number of installations reported on the card
async function openManageInstallations(eimRunner, { waitAfterClickMs = 0 } = {}) {
  const dashboardCard = await eimRunner.findByDataId("manage-versions-card");
  expect(
    dashboardCard,
    "Expected dashboard card to be shown on welcome page"
  ).to.not.be.false;
  expect(await dashboardCard.getText()).to.include(
    tGui("welcome.cards.manage.title")
  );
  const dashboardContent = await eimRunner.findByDataId(
    "manage-versions-description"
  );
  const text = await dashboardContent.getText();
  const numberMatch = text.match(/\d+/);
  const totalInstallations = numberMatch ? parseInt(numberMatch[0], 10) : 0;
  expect(totalInstallations, "Expected at least one installation").to.be.gte(1);
  const click = await eimRunner.clickByDataId("manage-versions-button");
  if (waitAfterClickMs) await sleep(waitAfterClickMs);
  expect(click, "Expected to click on Open Dashboard button").to.be.true;
  return totalInstallations;
}

async function openInstallerSetup(eimRunner) {
  await eimRunner.clickByDataId("new-installation-button");
  await sleep(2000);
  const header = await eimRunner.findByDataId("basic-installer-title");
  const text = await header.getText();
  expect(text, "Expected installation setup screen").to.equal(
    tGui("basicInstaller.title")
  );
}

// Poll for up to 45 minutes until one of `failureIds` or `completeId` is shown
async function waitForGUIInstallation(eimRunner, { failureIds, completeId }) {
  const startTime = Date.now();
  polling: while (Date.now() - startTime < 2700000) {
    for (const failureId of failureIds) {
      if (await eimRunner.findByDataId(failureId, 1000)) {
        logger.debug("failed!!!!");
        break polling;
      }
    }
    if (await eimRunner.findByDataId(completeId, 1000)) {
      logger.debug("Completed!!!");
      break;
    }
    await sleep(1000);
  }
  if (Date.now() - startTime >= 2700000) {
    logger.info("Installation timed out after 45 minutes");
  }
}

export {
  startGUIApp,
  stopGUIApp,
  registerGUIAppLifecycle,
  registerGUIFailureHooks,
  expectWelcomePage,
  openManageInstallations,
  openInstallerSetup,
  waitForGUIInstallation,
};

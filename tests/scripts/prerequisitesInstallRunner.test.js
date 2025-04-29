import { expect } from "chai";
import { describe, it, before, after, beforeEach, afterEach } from "mocha";
import { InteractiveCLITestRunner } from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";
import os from "os";
import path from "path";

/**
 * Setup the following environmental variables to execute this test:
 *
 * EIM_FILE_PATH to point to the eim application.
 *
 * use:
 * Windows: $env:<variable>="<value>"
 *
 */

let pathToEim;

if (process.env.EIM_FILE_PATH) {
  pathToEim = process.env.EIM_FILE_PATH;
} else {
  pathToEim = path.join(os.homedir(), "eim-cli/eim");
}

describe("Check Pre-requisites installation on Windows ->", function () {
  this.timeout(600000);
  let testRunner;

  beforeEach(async function () {
    this.timeout(5000);
    testRunner = new InteractiveCLITestRunner();
    try {
      await testRunner.start();
      testRunner.sendInput(`${pathToEim}\r`);
    } catch (error) {
      logger.info(`Error starting process: ${error}`);
      logger.info(` Error: ${error}`);
    }
  });

  afterEach(async function () {
    this.timeout(20000);
    if (this.currentTest.state === "failed") {
      logger.info(`Terminal output on failure: >>\r ${testRunner.output}`);
    }
    try {
      await testRunner.stop();
    } catch (error) {
      logger.info("Error to clean up terminal after test");
      logger.info(` Error: ${error}`);
    }
    testRunner = null;
  });

  it("1 - should install prerequisites and offer to install python and exit upon negative answer", async function () {
    logger.info(`Starting test - check python requirement`);
    this.timeout(240000);
    const promptRequisites = await testRunner.waitForOutput(
      "Do you want to install prerequisites"
    );

    expect(
      promptRequisites,
      "EIM did not offer to install the missing prerequisites"
    ).to.be.true;

    logger.info("Question to install prerequisites passed");

    testRunner.sendInput("y");

    const selectTargetQuestion = await testRunner.waitForOutput(
      "Please select all of the target platforms",
      240000
    );
    expect(
      selectTargetQuestion,
      "EIM did not ask to select target, error during python installation"
    ).to.be.true;
  });

  it("2 - should detect all prerequisites are installed", async function () {
    logger.info(`Starting test - all requirements installed`);
    this.timeout(22000);
    const selectTargetQuestion2 = await testRunner.waitForOutput(
      "Please select all of the target platforms",
      20000
    );
    expect(
      selectTargetQuestion2,
      "EIM did not ask to select target, error installing prerequisites"
    ).to.be.true;
  });
});

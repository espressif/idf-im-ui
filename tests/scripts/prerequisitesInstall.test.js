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
      testRunner.sendInput(`${pathToEim}`);
    } catch (error) {
      logger.info(`Error starting process: ${error}`);
      logger.info(` Error: ${error}`);
    }
  });

  afterEach(async function () {
    this.timeout(10000);
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

  it("should install prerequisites and offer to install python and exit upon negative answer", async function () {
    logger.info(`Starting test - check for missing requisites`);
    this.timeout(240000);
    const promptRequisites = await testRunner.waitForOutput(
      "Do you want to install prerequisites"
    );

    expect(
      promptRequisites,
      "EIM did not offer to install the missing prerequisites"
    ).to.be.true;

    logger.info("Question to install prerequisites passed");
    testRunner.output = "";
    testRunner.sendInput("y");

    const promptPython = await testRunner.waitForOutput(
      "Do you want to install Python",
      240000
    );

    expect(promptPython, "EIM did not offer to install python").to.be.true;
    expect(
      testRunner.output,
      "Error when installing the prerequisites"
    ).to.include("All prerequisites are satisfied");

    testRunner.sendInput("n");

    const terminalExited = await testRunner.waitForOutput(
      "Please install python3 with pip and SSL support and try again"
    );

    expect(terminalExited, "EIM did not fails after denying to install python")
      .to.be.true;
  });

  it("should install python and proceed with installation", async function () {
    logger.info(`Starting test - Check for python requisite`);
    this.timeout(240000);
    const promptPython2 = await testRunner.waitForOutput(
      "Do you want to install Python",
      240000
    );

    expect(promptPython2, "EIM did not offer to install python").to.be.true;
    expect(
      testRunner.output,
      "Error when installing the prerequisites"
    ).to.include("All prerequisites are satisfied");

    logger.info("Question to install python passed");
    testRunner.output = "";
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

  it("should detect all prerequisites are installed", async function () {
    logger.info(`Starting test - confirm no missing requisites`);
    this.timeout(20000);
    const selectTargetQuestion2 = await testRunner.waitForOutput(
      "Please select all of the target platforms",
      240000
    );
    expect(
      selectTargetQuestion2,
      "EIM did not ask to select target, error detecting prerequisites"
    ).to.be.true;
  });
});

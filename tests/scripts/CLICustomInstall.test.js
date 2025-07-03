import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";

export function runCLICustomInstallTest(pathToEim, args = []) {
  describe("1- Run custom ->", function () {
    let testRunner = null;

    before(async function () {
      logger.debug(
        `Installing custom IDF version with parameters ${args.join(" ")}`
      );
      this.timeout(5000);
      testRunner = new CLITestRunner();
      try {
        await testRunner.start();
      } catch {
        logger.info("Error to start terminal");
      }
    });

    afterEach(function () {
      if (this.currentTest.state === "failed") {
        logger.info(`Test failed: ${this.currentTest.title}`);
        logger.info(`Terminal output: >>\r ${testRunner.output.slice(-1000)}`);
        logger.debug(`Terminal output on failure: >>\r ${testRunner.output}`);
      }
    });

    after(async function () {
      logger.info("Custom installation routine completed");
      this.timeout(20000);
      try {
        await testRunner.stop();
      } catch (error) {
        logger.info("Error to clean up terminal after test");
        logger.info(` Error: ${error}`);
      }
    });

    /** Run installation with full parameters, no need to ask questions
     *
     * It is expected to have all requirements installed
     *
     */

    it("Should install IDF using specified parameters", async function () {
      logger.info(`Starting test - IDF custom installation`);
      testRunner.sendInput(`${pathToEim} install ${args.join(" ")}\r`);
      await new Promise((resolve) => setTimeout(resolve, 5000));
      if (!"-n true" in args) {
        const startTime = Date.now();
        while (Date.now() - startTime < 1800000) {
          if (Date.now() - testRunner.lastDataTimestamp >= 180000) {
            logger.info(">>>>>>>Exited due to Idle terminal!!!!!");
            break;
          }
          if (await testRunner.waitForOutput("panicked", 1000)) {
            logger.info(">>>>>>>Rust App failure!!!!");
            break;
          }
          if (
            await testRunner.waitForOutput(
              "Do you want to save the installer configuration",
              1000
            )
          ) {
            logger.info(">>>>>>>Completed!!!");
            break;
          }
          await new Promise((resolve) => setTimeout(resolve, 1000));
        }
        if (Date.now() - startTime >= 1800000) {
          logger.info("Installation timed out after 30 minutes");
        }

        expect(
          testRunner.output,
          "Failed to ask to save installation configuration - failure to install using full arguments on run time"
        ).to.include("Do you want to save the installer configuration");

        expect(
          testRunner.output,
          "Failed to download submodules, missing 'Finished fetching submodules'"
        ).to.include("Finished fetching submodules");

        logger.info("Installation completed");
        testRunner.output = "";
        testRunner.sendInput("n");
      }

      const startTime = Date.now();
      while (Date.now() - startTime < 1800000) {
        if (Date.now() - testRunner.lastDataTimestamp >= 180000) {
          logger.info(">>>>>>>Exited due to Idle terminal!!!!!");
          break;
        }
        if (await testRunner.waitForOutput("panicked", 1000)) {
          logger.info(">>>>>>>Rust App failure!!!!");
          break;
        }
        if (
          await testRunner.waitForOutput("Successfully installed IDF", 1000)
        ) {
          logger.info(">>>>>>>Completed!!!");
          break;
        }
        await new Promise((resolve) => setTimeout(resolve, 500));
      }
      if (Date.now() - startTime >= 1800000) {
        logger.info("Installation timed out after 30 minutes");
      }

      expect(
        testRunner.output,
        "Failed to complete installation, missing 'Successfully Installed IDF'"
      ).to.include("Successfully installed IDF");

      expect(
        testRunner.output,
        "Failed to complete installation, missing 'Now you can start using IDF tools'"
      ).to.include("Now you can start using IDF tools");

      if ("-r true" in args || "--recursive-submodules" in args) {
        expect(
          testRunner.output,
          "Failed to download submodules, missing 'Finished fetching submodules'"
        ).to.include("Finished fetching submodules");
      }
    });
  });
}

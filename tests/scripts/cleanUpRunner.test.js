import { expect } from "chai";
import { describe, it, after } from "mocha";
import logger from "../classes/logger.class.js";
import fs from "fs";

export function runCleanUp({
  id = 0,
  installFolder = null,
  toolsFolder = null,
  deleteAfterTest,
}) {
  describe(`${id}- Clean up EIM folders from Runner |`, function () {
    this.timeout(120000);

    after(function () {
      if (deleteAfterTest) {
        logger.info(
          `Running clean up function with Delete After Test = ${deleteAfterTest}`
        );
        try {
          if (installFolder) {
            fs.rmSync(installFolder, { recursive: true, force: true });
            logger.info(`Successfully deleted ${installFolder} folder`);
          }
          if (toolsFolder) {  
            fs.rmSync(toolsFolder, { recursive: true, force: true });
            logger.info(`Successfully deleted ${toolsFolder} folder`);
          }
        } catch (err) {
          logger.info(`Error deleting installation folders`);
          logger.info(`Error: ${err}`);
        }
      }
    });

    it("1- Install and Tools folder should exist", async function () {
      logger.info("Validating folders exist before deleting");
      if (installFolder) {
        expect(fs.existsSync(installFolder), "IDF Installation folder missing").to
          .be.true;
      }
      if (toolsFolder) {
        expect(fs.existsSync(toolsFolder), "IDF tools folder missing").to.be.true;
      }
    });
  });
}

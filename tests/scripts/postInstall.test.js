import { expect } from "chai";
import { describe, it, after } from "mocha";
import logger from "../classes/logger.class.js";
import fs from "fs";

export function runPostInstallCleanUp(installFolder) {
    describe("2- Clean UP after install ->", function () {
        after(function () {
            this.timeout(20000);
            logger.info("Starting cleanup");
            try {
                fs.rmSync(installFolder, { recursive: true, force: true });
                logger.info(`Successfully deleted ${installFolder}`);
            } catch (err) {
                logger.info(`Error deleting ${installFolder}`);
            }
        });

        it("Should run after installation is complete", function () {
            expect(true).to.be.true;
        });
    });
}

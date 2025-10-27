/**
 * Test runner to execute offline based on entries of archive index json file.
 * Entries should follow this format:
  {
    "filename": "<archive filename>",    //required file to download and test
    "version": "vx.x.x",                // IDF version contained in the archive
    "size": <number>,
    "platform": "windows-x64"           // Platform where this archive can be used
  },
 */

import { describe } from "mocha";
import { runCLICustomInstallTest } from "./scripts/CLICustomInstall.test.js";
import { runInstallVerification } from "./scripts/installationVerification.test.js";
import { runCleanUp } from "./scripts/cleanUpRunner.test.js";
import logger from "./classes/logger.class.js";
import {
  pathToEIMCLI,
  INSTALLFOLDER,
  TOOLSFOLDER,
  pathToBuildInfo,
} from "./config.js";
import path from "path";
import fs from "fs";

let buildInfo = [];

if (fs.existsSync(pathToBuildInfo)) {
  function readJsonFilesRecursively(dir) {
    const entries = fs.readdirSync(dir, { withFileTypes: true });
    entries.forEach((entry) => {
      const fullPath = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        readJsonFilesRecursively(fullPath);
      } else if (entry.isFile() && entry.name.endsWith(".json")) {
        try {
          const content = fs.readFileSync(fullPath, "utf8");
          const jsonData = JSON.parse(content);
          buildInfo.push(jsonData);
        } catch (err) {
          logger.error(`Failed to read or parse ${fullPath}: ${err.message}`);
        }
      }
    });
  }
  readJsonFilesRecursively(pathToBuildInfo);
} else {
  logger.error(`Directory not found: ${pathToBuildInfo}`);
}

logger.info("Running test script:", buildInfo);

testRun(buildInfo);

function testRun(archiveInfo) {
  // run tests for each archive information
  archiveInfo.forEach((info, idx) => {
    describe(`Test${idx + 1}- Test Package ${info.filename} |`, function () {
      logger.info(
        `Testing for IDF version: ${info.version} on platform: ${info.platform}`
      );
      // Set timeout to 100 minutes for installation tests
      // as downloading and installing can take a while
      this.timeout(6000000);

      runCLICustomInstallTest({
        id: `11`,
        pathToEIM: pathToEIMCLI,
        offlineIDFVersion: info.version,
        offlinePkgName: info.platform,
        testProxyMode: "block",
      });

      runInstallVerification({
        id: `12`,
        installFolder: INSTALLFOLDER,
        idfList: [info.version],
        toolsFolder: TOOLSFOLDER,
      });

      runCleanUp({
        id: `13`,
        installFolder: INSTALLFOLDER,
        toolsFolder: TOOLSFOLDER,
        deleteAfterTest: true,
      });
    });
  });
}

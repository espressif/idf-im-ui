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
  downloadOfflineArchive,
  runInDebug,
  pathToBuildInfo,
} from "./config.js";
import os from "os";
import path from "path";
import fs from "fs";

let buildInfo = {};

if (fs.existsSync(pathToBuildInfo)) {
  const files = fs
    .readdirSync(pathToBuildInfo)
    .filter((file) => file.endsWith(".json"));
  files.forEach((file) => {
    const filePath = path.join(pathToBuildInfo, file);
    try {
      const content = fs.readFileSync(filePath, "utf8");
      const jsonData = JSON.parse(content);
      buildInfo[file] = jsonData;
    } catch (err) {
      logger.error(`Failed to read or parse ${file}: ${err.message}`);
    }
  });
} else {
  logger.error(`Directory not found: ${pathToBuildInfo}`);
}

logger.info("Running test script:", buildInfo);

// testRun(buildInfo);

// function testRun(archiveInfo) {
//   // run tests for each archive information
//   archiveInfo.forEach(async (info) => {
//     const pathToOfflineArchive = await downloadOfflineArchive({
//       idfVersion: info.version,
//     });

//     const offlineArg = [
//       `${
//         runInDebug ? "-vvv " : ""
//       }--use-local-archive "${pathToOfflineArchive}"`,
//     ];

//     const deleteAfterTest = test.deleteAfterTest ?? true;
//     const testProxyMode = test.testProxyMode ?? "block";

//     describe(`Test${test.id} - ${test.name} ->`, function () {
//       this.timeout(6000000);

//       runCLICustomInstallTest({
//         pathToEim: pathToEIMCLI,
//         args: offlineArg,
//         testProxyMode,
//       });

//       runInstallVerification({
//         installFolder: INSTALLFOLDER,
//         idfList: [IDFDefaultVersion],
//         toolsFolder: TOOLSFOLDER,
//       });

//       runCleanUp({
//         installFolder: INSTALLFOLDER,
//         toolsFolder: TOOLSFOLDER,
//         deleteAfterTest,
//       });
//     });
//   });
// }

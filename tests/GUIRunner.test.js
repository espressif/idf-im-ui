/**
 * Test runner to execute test scripts based on entries of given json file.
 * Entries should follow this format:
 *     {
        "id": <number>,                 // an ID to correlate with the test report
        "type": "custom",               // test type is either "startup", "default" or "custom"
        "name": "<name>",               // A name for the test to correlate to logs and report
        "data": {                       // Only required for custom test type
            "targetList": "esp32s2",    // Which targets to install "esp32|esp32c6"
            "idfList": "v5.3.2",        // Which IDF version to install "v5.4|v.5.3.2"
            "installFolder": "<folder>", // Folder name to install idf (inside USER folder)
            "idfMirror": "github",      // Mirror to download IDF "github" or "jihulab"
            "toolsMirror": "github"     // Mirror to download tools "github", "dl_com" or "dl_cn"
        }

 */

import logger from "./classes/logger.class.js";
import fs from "fs";
import path from "path";
import os from "os";
import { describe } from "mocha";
import { runGUIStartupTest } from "./scripts/GUIStartup.test.js";
import { runGUISimplifiedInstallTest } from "./scripts/GUISimplifiedInstall.test.js";
import { runGUICustomInstallTest } from "./scripts/GUICustomInstall.test.js";
import { runInstallVerification } from "./scripts/installationVerification.test.js";
import { runCleanUp } from "./scripts/cleanUpRunner.test.js";
import { GUIDEFAULTVERSION, IDFDEFAULTINSTALLVERSION } from "./config.js";

logger.debug(`Filename Env variable: ${process.env.JSON_FILENAME}`);
logger.debug(`Execution folder: ${import.meta.dirname}`);

const jsonFilePath = path.join(
  import.meta.dirname,
  "suites",
  `${process.env.JSON_FILENAME}.json`
);
const testScript = JSON.parse(fs.readFileSync(jsonFilePath, "utf-8"));
logger.info(`Running test script: ${jsonFilePath}`);

testRun(testScript);

function testRun(script) {
  let pathToEIM;
  if (process.env.EIM_GUI_PATH) {
    pathToEIM = process.env.EIM_GUI_PATH;
  } else {
    os.platform() !== "win32"
      ? (pathToEIM = path.resolve(os.homedir(), "eim-gui", "eim"))
      : (pathToEIM = path.resolve(os.homedir(), "eim-gui", "eim.exe"));
  }

  const EIMVersion = process.env.EIM_GUI_VERSION || GUIDEFAULTVERSION;

  const IDFDefaultVersion =
    process.env.IDF_VERSION && process.env.IDF_VERSION !== "null"
      ? process.env.IDF_VERSION
      : IDFDEFAULTINSTALLVERSION;

  const INSTALLFOLDER =
    os.platform() !== "win32"
      ? path.join(os.homedir(), `.espressif`)
      : `C:\\esp`;

  const TOOLSFOLDER =
    os.platform() !== "win32"
      ? path.join(os.homedir(), `.espressif`)
      : `C:\\Espressif`;

  script.forEach((test) => {
    if (test.type === "startup") {
      //routine for startup test script
      describe(`Test${test.id} - ${test.name} ->`, function () {
        this.timeout(60000);

        runGUIStartupTest(test.id, pathToEIM, EIMVersion);
      });
    } else if (test.type === "default") {
      //routine for default simplified installation

      const deleteAfterTest = test.deleteAfterTest || true;

      describe(`Test${test.id} - ${test.name} ->`, function () {
        runGUISimplifiedInstallTest(test.id, pathToEIM);
        runInstallVerification({
          installFolder: INSTALLFOLDER,
          idfList: [IDFDefaultVersion],
          toolsFolder: TOOLSFOLDER,
        });
        runCleanUp({
          installFolder: INSTALLFOLDER,
          toolsFolder: TOOLSFOLDER,
          deleteAfterTest,
        });
      });
    } else if (test.type === "custom") {
      //routine for expert install with custom settings

      let installFolder = test.data.installFolder
        ? path.join(os.homedir(), test.data.installFolder)
        : INSTALLFOLDER;

      const targetList = test.data.targetList
        ? test.data.targetList.split("|")
        : ["All"];
      const idfVersionList = test.data.idfList
        ? test.data.idfList.split("|")
        : [IDFDefaultVersion];

      const toolsMirror = test.data.toolsMirror || "github";

      const IDFMirror = test.data.idfMirror || "github";

      const deleteAfterTest = test.deleteAfterTest || true;

      describe(`Test${test.id} - ${test.name} ->`, function () {
        runGUICustomInstallTest(
          test.id,
          pathToEIM,
          installFolder,
          targetList,
          idfVersionList,
          toolsMirror,
          IDFMirror
        );
        runInstallVerification({
          installFolder,
          idfList: idfVersionList,
          targetList,
          toolsFolder: TOOLSFOLDER,
        });

        runCleanUp({
          installFolder,
          toolsFolder: TOOLSFOLDER,
          deleteAfterTest,
        });
      });
    }
  });
}

/**
 * Test runner to execute test scripts based on entries of given json file.
 * Entries should follow this format:
 *     {
        "id": <number>,                 // an ID to correlate with the test report
        "type": "custom",               // test type is either "prerequisites", "arguments", "default" or "custom"
        "name": "<name>",               // A name for the test to correlate to logs and report
        "data": {                       // Only required for custom test type
            "targetList": "esp32s2",    // Which targets to install "esp32|esp32c6"
            "idfList": "v5.3.2",        // Which IDF version to install "v5.4|v.5.3.2"
            "installFolder": "<folder>", // Folder name to install idf (inside USER folder)
            "idfMirror": "github",      // Mirror to download IDF "github" or "jihulab"
            "toolsMirror": "github"     // Mirror to download tools "github", "dl_com" or "dl_cn"
            "recursive": "false",        // Whether to prevent downloading submodules (set to true if omitted)
            "nonInteractive": "false"    // Whether to prevent running in non-interactive mode (set to true if omitted)
        }

 */

import { describe } from "mocha";
import { runCLIPrerequisitesTest } from "./scripts/CLIPrerequisites.test.js";
import { runCLIArgumentsTest } from "./scripts/CLIArguments.test.js";
import { runCLIWizardInstallTest } from "./scripts/CLIWizardInstall.test.js";
import { runCLICustomInstallTest } from "./scripts/CLICustomInstall.test.js";
import { runInstallVerification } from "./scripts/installationVerification.test.js";
import logger from "./classes/logger.class.js";
import {
  IDFMIRRORS,
  TOOLSMIRRORS,
  CLIDEFAULTVERSION,
  IDFDEFAULTINSTALLVERSION,
} from "./config.js";
import os from "os";
import path from "path";
import fs from "fs";

const jsonFilePath = path.join(
  import.meta.dirname,
  "suites",
  `${process.env.JSON_FILENAME}.json`
);
const testScript = JSON.parse(fs.readFileSync(jsonFilePath, "utf-8"));
logger.info(`Running test script: ${jsonFilePath}`);

testRun(testScript);

function testRun(jsonScript) {
  const PATHTOEIM =
    process.env.EIM_FILE_PATH || path.join(os.homedir(), "eim-cli/eim");

  const EIMVERSION = process.env.EIM_VERSION || CLIDEFAULTVERSION;

  const IDFDefaultVersion =
    process.env.IDF_VERSION && process.env.IDF_VERSION !== "null"
      ? process.env.IDF_VERSION
      : IDFDEFAULTINSTALLVERSION;

  // Test Runs
  jsonScript.forEach((test) => {
    if (test.type === "prerequisites") {
      //route for prerequisites tests

      describe(`Test${test.id} - ${test.name} ->`, function () {
        this.timeout(20000);

        runCLIPrerequisitesTest(PATHTOEIM);
      });
    } else if (test.type === "arguments") {
      //routine for arguments tests

      describe(`Test${test.id} - ${test.name} ->`, function () {
        this.timeout(20000);

        runCLIArgumentsTest(PATHTOEIM, EIMVERSION);
      });
    } else if (test.type === "default") {
      //routine for default installation tests

      const installFolder =
        os.platform() !== "win32"
          ? path.join(os.homedir(), `.espressif`)
          : `C:\\esp`;

      describe(`Test${test.id} - ${test.name} ->`, function () {
        this.timeout(6000000);

        runCLIWizardInstallTest(PATHTOEIM);

        runInstallVerification({ installFolder, idfList: [IDFDefaultVersion] });
      });
    } else if (test.type === "custom") {
      //routine for custom installation tests
      let installFolder;
      if (test.data.installFolder) {
        installFolder = path.join(os.homedir(), test.data.installFolder);
      } else {
        installFolder =
          os.platform() !== "win32"
            ? path.join(os.homedir(), `.espressif`)
            : `C:\\esp`;
      }

      const targetList = test.data.targetList
        ? test.data.targetList.split("|")
        : ["esp32"];

      const idfVersionList = test.data.idfList
        ? test.data.idfList.split("|")
        : [IDFDefaultVersion];

      let installArgs = [];

      test.data.installFolder && installArgs.push(`-p ${installFolder}`);

      test.data.targetList && installArgs.push(`-t ${targetList.join(",")}`);

      test.data.idfList && installArgs.push(`-i ${idfVersionList.join(",")}`);

      test.data.toolsMirror &&
        installArgs.push(`-m ${TOOLSMIRRORS[test.data.toolsMirror]}`);

      test.data.idfMirror &&
        installArgs.push(`--idf-mirror ${IDFMIRRORS[test.data.idfMirror]}`);

      test.data.recursive && installArgs.push(`-r ${test.data.recursive}`);

      test.data.nonInteractive &&
        installArgs.push(`-n ${test.data.nonInteractive}`);

      describe(`Test${test.id} - ${test.name} ->`, function () {
        this.timeout(6000000);

        runCLICustomInstallTest(PATHTOEIM, installArgs);

        runInstallVerification({
          installFolder,
          idfList: idfVersionList,
          targetList,
        });
      });
    }
  });
}

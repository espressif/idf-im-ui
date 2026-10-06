/**
 * Helpers shared by the test runners to read a suite JSON file and turn a
 * suite entry into the settings passed to the test scripts.
 */

import logger from "../classes/logger.class.js";
import {
  IDFMIRRORS,
  TOOLSMIRRORS,
  PYPIMIRRORS,
  IDFDefaultVersion,
  INSTALLFOLDER,
  runInDebug,
  resolveIdfToken,
} from "../config.js";
import os from "os";
import path from "path";
import fs from "fs";

// Read the suite file named by the JSON_FILENAME env variable from tests/suites
function loadTestSuite() {
  const jsonFilePath = path.join(
    import.meta.dirname,
    "..",
    "suites",
    `${process.env.JSON_FILENAME}.json`
  );
  const testScript = JSON.parse(fs.readFileSync(jsonFilePath, "utf-8"));
  logger.info(`Running test script: ${jsonFilePath}`);
  return testScript;
}

const resolveInstallFolder = (data) =>
  data.installFolder ? path.join(os.homedir(), data.installFolder) : INSTALLFOLDER;

const parseTargetList = (data, defaultTargets) =>
  data.targetList ? data.targetList.split("|") : defaultTargets;

const resolveIdfList = (data) =>
  (data.idfList ? data.idfList.split("|") : [IDFDefaultVersion]).map((idf) =>
    resolveIdfToken(idf)
  );

const getCleanupAndProxyOptions = (test, defaultProxyMode = false) => ({
  deleteAfterTest: test.deleteAfterTest ?? true,
  testProxyMode: test.testProxyMode ?? defaultProxyMode,
  proxyBlockList: test.proxyBlockList ?? [],
});

// Build the `eim install` arguments for the options set in the suite entry data
function buildCLIInstallArgs(
  data,
  { installFolder, targetList, idfList, quoteInstallPathOnWindows = false }
) {
  const installArgs = [];
  runInDebug && installArgs.push("-vvv");
  data.installFolder &&
    installArgs.push(
      quoteInstallPathOnWindows && os.platform() === "win32"
        ? `-p "${installFolder}"`
        : `-p ${installFolder}`
    );
  data.targetList && installArgs.push(`-t ${targetList.join(",")}`);
  data.idfList && installArgs.push(`-i ${idfList.join(",")}`);
  data.toolsMirror &&
    installArgs.push(
      `-m ${TOOLSMIRRORS[data.toolsMirror] || "https://github.com"}`
    );
  data.idfMirror &&
    installArgs.push(
      `--idf-mirror ${IDFMIRRORS[data.idfMirror] || "https://github.com"}`
    );
  data.pypiMirror &&
    installArgs.push(
      `--pypi-mirror ${PYPIMIRRORS[data.pypiMirror] || "https://pypi.org/simple"}`
    );
  data.recursive && installArgs.push(`-r ${data.recursive}`);
  data.nonInteractive && installArgs.push(`-n ${data.nonInteractive}`);
  return installArgs;
}

// Settings for suite entries that run `eim install` with arguments built from `data`
function getCLIInstallSettings(test, { quoteInstallPathOnWindows = false } = {}) {
  const options = getCleanupAndProxyOptions(test);
  const installFolder = resolveInstallFolder(test.data);
  const targetList = parseTargetList(test.data, ["esp32"]);
  const idfList = resolveIdfList(test.data);
  const installArgs = buildCLIInstallArgs(test.data, {
    installFolder,
    targetList,
    idfList,
    quoteInstallPathOnWindows,
  });
  return { ...options, installFolder, targetList, idfList, installArgs };
}

export {
  loadTestSuite,
  resolveInstallFolder,
  parseTargetList,
  resolveIdfList,
  getCleanupAndProxyOptions,
  buildCLIInstallArgs,
  getCLIInstallSettings,
};

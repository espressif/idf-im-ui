/**
 * General test configuration file.
 * Contains constants and configurations used across tests.
 *
 */

import { log } from "console";
import logger from "./classes/logger.class.js";
import os from "os";
import path from "path";

// Define default values for offline tests
let IDFDefaultVersion = "v5.5.1";
let IDFAvailableVersions = { development: "master" };
let IDFDefaultVersionIndex = 0;
let availableTargets = [
  "esp32",
  "esp32s2",
  "esp32s3",
  "esp32c2",
  "esp32c3",
  "esp32c5",
  "esp32c6",
  "esp32h2",
  "esp32p4",
];

// Replace default values with Espressif server data.

// IDF versions are provided by json file available at https://dl.espressif.com/dl/esp-idf/idf_versions.json
const url = "https://dl.espressif.com/dl/esp-idf/idf_versions.json";

try {
  const res = await fetch(url);
  if (res.ok) {
    const data = await res.json();
    const idfVersions = data.VERSIONS;
    const idfTargets = data.IDF_TARGETS;
    if (idfVersions && idfVersions.length > 0) {
      IDFDefaultVersion =
        idfVersions.find(
          (version) => version.old !== true && version.pre_release !== true && version.name !== 'latest'
        )?.name || IDFDefaultVersion;
      logger.info(`IDF Default Version set to: ${IDFDefaultVersion}`);
      let IDFValidVersions = [...idfVersions.filter((v)=>v.old!==true && v.name !== 'latest').map((v)=>v.name)];
      IDFDefaultVersionIndex = IDFValidVersions.indexOf(IDFDefaultVersion) === -1? IDFDefaultVersionIndex: IDFValidVersions.indexOf(IDFDefaultVersion);

      const validIdfVersions = idfVersions.filter(
        (v) => v.old !== true && v.name !== "latest",
      );
      IDFAvailableVersions.stable = validIdfVersions
        .filter((v) => v.pre_release !== true)
        .map((v) => v.name);
      IDFAvailableVersions.prerelease = validIdfVersions
        .filter((v) => v.pre_release === true)
        .map((v) => v.name);

      logger.info(
        `Available IDF Versions: ${JSON.stringify(IDFAvailableVersions)}`,
      );
      logger.info(
        `IDF Default Version Index set to: ${IDFDefaultVersionIndex}`,
      );
    } else {
      logger.info("No IDF versions found in the response.");
    }
    if (idfTargets && idfTargets.length > 0) {
      availableTargets = idfTargets.map((target) => target.value);
      logger.info(`Available IDF Targets: ${availableTargets.join(", ")}`);
    } else {
      logger.info("No IDF targets found in the response.");
    }
  } else {
    logger.info(`Failed to fetch IDF versions: ${res.statusText}`);
  }
} catch (error) {
  logger.error(`Error fetching IDF versions file: ${error.message}`);
}

const IDFMIRRORS = {
  github: "https://github.com",
  jihulab: "https://jihulab.com/esp-mirror",
};
const TOOLSMIRRORS = {
  github: "https://github.com",
  dl_com: "https://dl.espressif.com/github_assets",
  dl_cn: "https://dl.espressif.cn/github_assets",
};
const PYPIMIRRORS = {
  pypi_org: "https://pypi.org/simple",
  pypi_aliyun: "https://mirrors.aliyun.com/pypi/simple",
  pypi_tsinghua: "https://pypi.tuna.tsinghua.edu.cn/simple",
  pypi_ustc: "https://pypi.mirrors.ustc.edu.cn/simple"
};

// Default versions for EIM CLI and GUI for offline testing
const EIMCLIVersion = process.env.EIM_CLI_VERSION || "eim 0.5.0";
const EIMGUIVersion = process.env.EIM_GUI_VERSION || "0.5.0";
logger.info(
  `EIM CLI version set to: ${EIMCLIVersion} and GUI to: ${EIMGUIVersion}`
);

// Default path to EIM CLI and GUI executables for offline testing
//Should use path provided by environment variables or default to home directory

const getEIMPath = (pathFromCI, defaultFolder) =>
  pathFromCI ||
  path.join(
    os.homedir(),
    defaultFolder,
    os.platform() === "win32" ? "eim.exe" : "eim"
  );

const pathToEIMCLI = getEIMPath(process.env.EIM_CLI_PATH, "eim-cli");
const pathToEIMGUI = getEIMPath(process.env.EIM_GUI_PATH, "eim-gui");
logger.info(
  `Path to EIM CLI set to: ${pathToEIMCLI} and GUI to: ${pathToEIMGUI}`
);

// Get path for build info file
const pathToBuildInfo = process.env.BUILD_INFO_PATH || "~/build_info";
logger.info(`Path to build info set to: ${pathToBuildInfo}`);

// Default installation folder
const INSTALLFOLDER =
  os.platform() !== "win32" ? path.join(os.homedir(), `.espressif`) : `C:\\esp`;
logger.info(`Installation folder set to: ${INSTALLFOLDER}`);

// Default tools folder
const TOOLSFOLDER =
  os.platform() !== "win32"
    ? path.join(os.homedir(), `.espressif`)
    : `C:\\Espressif`;
logger.info(`Tools folder set to: ${TOOLSFOLDER}`);

// Enable running EIM in debug mode
const runInDebug = (process.env.DEBUG || "false") === "true";
logger.info(`Run in debug mode: ${runInDebug}`);

// Define versions of python Wheels included in the offline package
let pythonWheelsVersion = ["311"]
if (os.platform() !== "win32") {
  pythonWheelsVersion.push("310", "312", "313", "314");
}
logger.info(`Python wheels versions included: ${pythonWheelsVersion.join(", ")}`)

//Capture list of prerequisites from environment variables
const preRequisitesList = {
  ubuntu: ["git","wget", "flex", "bison", "gperf", "ccache", "libffi-dev", "libssl-dev", "dfu-util", "cmake","libusb-1.0-0"],
  debian: ["git","wget", "flex", "bison", "gperf", "ccache", "libffi-dev", "libssl-dev", "dfu-util", "cmake","libusb-1.0-0"],
  fedora:["git","wget", "flex", "bison", "gperf", "ccache", "libffi-devel", "openssl-devel", "dfu-util", "cmake"], //"libusb1-devel" is installed by default
  archlinux: ["git","wget", "flex", "bison", "gperf", "ccache", "dfu-util", "cmake"], //"libffi", "openssl", "libusb" are installed by default
  opensuse: ["git","wget", "flex", "bison", "gperf", "ccache", "libffi-devel", "libopenssl-devel", "dfu-util", "cmake"], //"libusb-1_0-0" is installed by default
  macos:["dfu-util", "cmake"],
}
const prerequisites = process.env.PREREQUISITES_OS?
  preRequisitesList[process.env.PREREQUISITES_OS.split(":")[0].toLowerCase()] || [] : [];
logger.info(`Prerequisites set to: ${prerequisites.join(", ")}`);

export {
  IDFMIRRORS,
  TOOLSMIRRORS,
  PYPIMIRRORS,
  IDFDefaultVersion,
  IDFDefaultVersionIndex,
  IDFAvailableVersions,
  availableTargets,
  pathToEIMCLI,
  pathToEIMGUI,
  pathToBuildInfo,
  EIMGUIVersion,
  EIMCLIVersion,
  INSTALLFOLDER,
  TOOLSFOLDER,
  runInDebug,
  pythonWheelsVersion,
  prerequisites,
};

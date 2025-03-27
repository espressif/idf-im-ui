import { describe } from "mocha";
import { runArgumentsTests } from "../script/commandLineArguments.test.js";
import { runInstallWizardTests } from "../script/installWizard.test.js";
import { runInstallCustom } from "../script/installCustom.test.js";
import { runPostInstallTest } from "../script/postInstall.test.js";
import logger from "../classes/logger.class.js";
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
    const IDFMIRRORS = {
        github: "https://github.com",
        jihulab: "https://jihulab.com/esp-mirror",
    };
    const TOOLSMIRRORS = {
        github: "https://github.com",
        dl_com: "https://dl.espressif.com/github_assets",
        dl_cn: "https://dl.espressif.cn/github_assets",
    };

    const PATHTOEIM =
        process.env.EIM_FILE_PATH || path.join(os.homedir(), "eim-cli/eim");

    const EIMVERSION = process.env.EIM_VERSION || "eim 0.1.6";

    const IDFDEFAULTVERSION =
        process.env.IDF_VERSION & (process.env.IDF_VERSION !== "null")
            ? process.env.IDF_VERSION
            : "v5.4";

    // Test Runs
    jsonScript.forEach((test) => {
        if (test.type === "arguments") {
            //routine for arguments tests

            describe(`${test.id} - EIM command line arguments ->`, function () {
                this.timeout(20000);

                runArgumentsTests(PATHTOEIM, EIMVERSION);
            });
        } else if (test.type === "default") {
            //routine for default installation tests

            const installFolder =
                os.platform() !== "win32"
                    ? path.join(os.homedir(), `.espressif`)
                    : `C:\\esp`;

            const pathToIDFScript =
                os.platform() !== "win32"
                    ? path.join(
                          installFolder,
                          `activate_idf_${IDFDEFAULTVERSION}.sh`
                      )
                    : path.join(
                          installFolder,
                          IDFDEFAULTVERSION,
                          "Microsoft.PowerShell_profile.ps1"
                      );

            describe(`${test.id} - Installation manager default installation ->`, function () {
                this.timeout(2400000);

                runInstallWizardTests(PATHTOEIM);

                runPostInstallTest(pathToIDFScript, installFolder);
            });
        } else if (test.type === "custom") {
            //routine for custom installation tests
            let installFolder;
            if (test.data.installFolder) {
                installFolder = path.join(
                    os.homedir(),
                    test.data.installFolder
                );
            } else {
                installFolder =
                    os.platform() !== "win32"
                        ? path.join(os.homedir(), `.espressif`)
                        : `C:\\esp`;
            }

            const targetList = test.data.targetList || "esp32";
            const idfVersionList = test.data.idfList || IDFDEFAULTVERSION;

            let installArgs = [];

            test.data.installFolder && installArgs.push(`-p ${installFolder}`);

            test.data.targetList &&
                installArgs.push(
                    `-t ${test.data.targetList.split("|").join(",")}`
                );

            test.data.idfList &&
                installArgs.push(
                    `-i ${test.data.idfList.split("|").join(",")}`
                );

            test.data.toolsMirror &&
                installArgs.push(`-m ${TOOLSMIRRORS[test.data.toolsMirror]}`);

            test.data.idfMirror &&
                installArgs.push(
                    `--idf-mirror ${IDFMIRRORS[test.data.idfMirror]}`
                );

            test.data.recursive &&
                installArgs.push(`-r ${test.data.recursive}`);

            test.data.nonInteractive &&
                installArgs.push(`-n ${test.data.nonInteractive}`);

            const pathToIDFScript =
                os.platform() !== "win32"
                    ? path.join(
                          installFolder,
                          `activate_idf_${idfVersionList.split("|")[0]}.sh`
                      )
                    : path.join(
                          installFolder,
                          idfVersionList.split("|")[0],
                          `Microsoft.PowerShell_profile.ps1`
                      );
            describe(`${test.id} - Installation using custom settings -> ${test.name} ->`, function () {
                this.timeout(2400000);

                runInstallCustom(PATHTOEIM, installArgs);

                runPostInstallTest(
                    pathToIDFScript,
                    installFolder,
                    targetList.split("|")[0]
                );
            });
        }
    });
}

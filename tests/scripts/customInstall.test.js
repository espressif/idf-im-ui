import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import { By, Key } from "selenium-webdriver";
import { EIMRunner } from "../classes/tauriRunner.class.js";
import logger from "../classes/logger.class.js";
import os from "os";

export function runInstallCustom(
    pathToEIM,
    installFolder,
    targetList,
    idfVersionList,
    toolsMirror,
    idfMirror
) {
    let eimRunner = "";

    describe("EIM expert Installation", () => {
        let customInstallFailed = false;

        before(async function () {
            this.timeout(30000);
            eimRunner = new EIMRunner(pathToEIM);
            try {
                await eimRunner.launchEIM();
            } catch (err) {
                logger.info("Error starting EIM application");
            }
        });

        beforeEach(async function () {
            if (customInstallFailed) {
                logger.info("Test failed, skipping next tests");
                this.skip();
            }
        });

        afterEach(async function () {
            if (this.currentTest.state === "failed") {
                await eimRunner.takeScreenshot(`${this.currentTest.title}.png`);
                logger.info(
                    `Screenshot saved as ${this.currentTest.title}.png`
                );
                customInstallFailed = true;
            }
        });

        after(async function () {
            this.timeout(5000);
            try {
                await eimRunner.closeEIM();
            } catch (error) {
                logger.info("Error to close IEM application");
            }
        });

        it("Should show welcome page", async function () {
            this.timeout(10000);
            // Wait for the header to be present
            const header = await eimRunner.findByCSS("h1");
            const text = await header.getText();
            expect(text).to.equal("Welcome to ESP-IDF Installation Manager!");
        });

        it("Should show expert installation option", async function () {
            this.timeout(10000);
            await eimRunner.clickButton("Get Started");
            const expert = await eimRunner.findByDataId("expert-title");
            expect(await expert.getText()).to.equal("Expert Installation");
            expect(await expert.isDisplayed()).to.be.true;
        });

        it("Should check prerequisites", async function () {
            this.timeout(10000);
            await eimRunner.clickButton("Start Expert Setup");
            await new Promise((resolve) => setTimeout(resolve, 5000));
            const prerequisitesList = await eimRunner.findByDataId(
                "prerequisites-items-list"
            );
            const requisitesList = await prerequisitesList.getText();
            expect(requisitesList).to.not.be.empty;
            expect(requisitesList).to.not.include("❌");
            if (os.platform() === "win32") {
                let expectedRequisites = ["git", "cmake", "ninja"];
                expectedRequisites.forEach((requisite) =>
                    expect(requisitesList).to.include(requisite)
                );
            } else {
                let expectedRequisites = [
                    "git",
                    "cmake",
                    "ninja",
                    "wget",
                    "flex",
                    "bison",
                    "gperf",
                    "ccache",
                    "libffi-dev",
                    "libssl-dev",
                    "dfu-util",
                    "libusb-1.0-0",
                ];
                expectedRequisites.forEach((requisite) =>
                    expect(requisitesList).to.include(requisite)
                );
            }
        });

        it("Should check python installation", async function () {
            this.timeout(10000);
            await eimRunner.clickButton("Continue to Next Step");
            await new Promise((resolve) => setTimeout(resolve, 5000));
            const result = await eimRunner.findByDataId("python-check-result");
            expect(await result.getText()).to.include(
                "Python Environment Ready"
            );
        });

        it("Should show targets list", async function () {
            this.timeout(10000);
            await eimRunner.clickButton("Continue to Next Step");
            await new Promise((resolve) => setTimeout(resolve, 2000));
            const targetsList = await eimRunner.findByDataId("targets-grid");
            const targetsText = await targetsList.getText();
            let expected = [
                "esp32",
                "esp32c2",
                "esp32c3",
                "esp32c5",
                "esp32c6",
                "esp32h2",
                "esp32p4",
                "esp32s2",
                "esp32s3",
            ];
            expected.forEach((target) =>
                expect(targetsText).to.include(target)
            );
            let targetAll = await eimRunner.findByText("All");
            expect(
                await targetAll.findElement(By.css("Div")).getAttribute("class")
            ).to.include("checked");
            let targetESP32 = await eimRunner.findByDataId("target-item-esp32");
            expect(await targetESP32.getAttribute("class")).to.not.include(
                "selected"
            );
            await eimRunner.clickButton("esp32");
            expect(
                await targetAll.findElement(By.css("Div")).getAttribute("class")
            ).not.to.include("checked");
            expect(await targetESP32.getAttribute("class")).to.include(
                "selected"
            );
            await eimRunner.clickButton("esp32");
            expect(
                await targetAll.findElement(By.css("Div")).getAttribute("class")
            ).to.include("checked");
            await eimRunner.clickButton("All");

            targetList.forEach(async function (target) {
                await eimRunner.clickButton(target);
                let selectedTarget = await eimRunner.findByDataId(
                    `target-item-${target}`
                );
                expect(
                    await selectedTarget.getAttribute("class")
                ).to.not.include("selected");
            });
        });

        it("Should show IDF version list", async function () {
            this.timeout(10000);
            await eimRunner.clickButton("Continue with Selected Targets");
            await new Promise((resolve) => setTimeout(resolve, 4000));
            const IDFList = await eimRunner.findByDataId("versions-grid");
            const IDFListText = await IDFList.getText();
            logger.info(IDFListText);
            let expected = ["v5.4", "v5.3.2", "v5.1.5", "master"];
            expected.forEach((version) =>
                expect(IDFListText).to.include(version)
            );
            let IDFMaster = await eimRunner.findByDataId("version-item-master");
            expect(await IDFMaster.getAttribute("class")).to.not.include(
                "selected"
            );
            await eimRunner.clickButton("master");
            expect(await IDFMaster.getAttribute("class")).to.include(
                "selected"
            );
            const selectedMaster = await eimRunner.findByDataId(
                "selected-tag-master"
            );
            await selectedMaster.findElement(By.css("button")).click();
            expect(await IDFMaster.getAttribute("class")).to.not.include(
                "selected"
            );

            idfVersionList.forEach(
                async (version) => await eimRunner.clickButton(version)
            );

            const selectedVersions = await eimRunner.findByText(
                "Selected versions:"
            );
            const selectedVersionsText = await selectedVersions.getText();
            expected = ["Selected versions:", ...idfVersionList];
            expected.forEach(async (substring) =>
                expect(selectedVersionsText).to.include(substring)
            );
        });

        it("Should show IDF download mirrors", async function () {
            this.timeout(10000);
            await eimRunner.clickButton("Continue Installation");
            await new Promise((resolve) => setTimeout(resolve, 2000));
            const IDFMirrors = await eimRunner.findByDataId(
                "idf-mirror-radio-group"
            );
            let idfMList = await IDFMirrors.getText();
            let expectedMirrors = [
                "https://github.com",
                "https://jihulab.com/esp-mirror",
            ];
            expectedMirrors.forEach((mirror) =>
                expect(idfMList).to.include(mirror)
            );

            let githubMirror = await eimRunner.findByDataId(
                "idf-mirror-option-https://github.com"
            );
            let jihulabMirror = await eimRunner.findByDataId(
                "idf-mirror-option-https://jihulab.com/esp-mirror"
            );
            expect(await githubMirror.getAttribute("class")).to.include(
                "selected"
            );
            expect(await jihulabMirror.getAttribute("class")).to.not.include(
                "selected"
            );
            await jihulabMirror.findElement(By.css("input")).click();
            expect(await githubMirror.getAttribute("class")).to.not.include(
                "selected"
            );
            expect(await jihulabMirror.getAttribute("class")).to.include(
                "selected"
            );

            idfMirror === "github" &&
                (await githubMirror.findElement(By.css("input")).click());
            idfMirror === "jihulab" &&
                (await jihulabMirror.findElement(By.css("input")).click());
        });

        it("Should show tools download mirrors", async function () {
            this.timeout(10000);
            const toolsMirrors = await eimRunner.findByDataId(
                "tools-mirror-radio-group"
            );
            let toolsMList = await toolsMirrors.getText();
            let expectedMirrors = [
                "https://github.com",
                "https://dl.espressif.com/github_assets",
                "https://dl.espressif.cn/github_assets",
            ];
            expectedMirrors.forEach((mirror) =>
                expect(toolsMList).to.include(mirror)
            );
            let githubMirror = await eimRunner.findByDataId(
                "tools-mirror-option-https://github.com"
            );
            let espressifComMirror = await eimRunner.findByDataId(
                "tools-mirror-option-https://dl.espressif.com/github_assets"
            );
            let espressifCnMirror = await eimRunner.findByDataId(
                "tools-mirror-option-https://dl.espressif.cn/github_assets"
            );
            expect(await githubMirror.getAttribute("class")).to.include(
                "selected"
            );
            expect(
                await espressifComMirror.getAttribute("class")
            ).to.not.include("selected");
            expect(
                await espressifCnMirror.getAttribute("class")
            ).to.not.include("selected");
            await espressifComMirror.findElement(By.css("input")).click();
            expect(await githubMirror.getAttribute("class")).to.not.include(
                "selected"
            );
            expect(await espressifComMirror.getAttribute("class")).to.include(
                "selected"
            );
            expect(
                await espressifCnMirror.getAttribute("class")
            ).to.not.include("selected");
            await espressifCnMirror.findElement(By.css("input")).click();
            expect(await githubMirror.getAttribute("class")).to.not.include(
                "selected"
            );
            expect(
                await espressifComMirror.getAttribute("class")
            ).to.not.include("selected");
            expect(await espressifCnMirror.getAttribute("class")).to.include(
                "selected"
            );

            toolsMirror === "github" &&
                (await githubMirror.findElement(By.css("input")).click());
            toolsMirror === "dl_com" &&
                (await espressifComMirror.findElement(By.css("input")).click());
            toolsMirror === "dl_cn" &&
                (await espressifCnMirror.findElement(By.css("input")).click());
        });

        it("Should show installation path", async function () {
            this.timeout(10000);
            await eimRunner.clickButton("Continue with Selected Mirrors");
            await new Promise((resolve) => setTimeout(resolve, 2000));
            const installPath = await eimRunner.findByDataId("path-info-title");
            expect(await installPath.getText()).to.equal(
                "ESP-IDF Installation Directory"
            );
            expect(await installPath.isDisplayed()).to.be.true;
            const pathInput = await eimRunner.findByDataId(
                "installation-path-input"
            );
            const input = await pathInput.findElement(By.css("input"));
            const defaultInput =
                os.platform() === "win32" ? "C:\\esp" : "/.espressif";
            expect(await input.getAttribute("value")).to.include(defaultInput);
            await input.sendKeys(Key.CONTROL + "a");
            await input.sendKeys(Key.CONTROL + "a");
            await input.sendKeys(Key.BACK_SPACE);
            await input.sendKeys(installFolder);
            expect(await input.getAttribute("value")).to.equal(installFolder);
        });

        it("Should show installation summary", async function () {
            this.timeout(10000);
            await eimRunner.clickButton("Continue");
            const versionSummary = await eimRunner.findByDataId(
                "versions-info"
            );
            expect(await versionSummary.getText()).to.include(
                "Installing ESP-IDF Versions"
            );
            const selectedVersions = await eimRunner.findByDataId(
                "version-chips"
            );
            const selectedVersionsText = await selectedVersions.getText();
            idfVersionList.forEach(async (idfVersion) =>
                expect(selectedVersions).to.include(idfVersion)
            );
        });

        it("Should install IDF", async function () {
            this.timeout(1300000);

            try {
                await eimRunner.clickButton("Start Installation");
                const installing = await eimRunner.findByText(
                    "Installation Progress"
                );
                expect(await installing.isDisplayed()).to.be.true;
                const startTime = Date.now();

                while (Date.now() - startTime < 1800000) {
                    if (
                        await eimRunner.findByText("Installation Failed", 1000)
                    ) {
                        logger.debug("failed!!!!");
                        break;
                    }
                    if (
                        await eimRunner.findByText(
                            "Complete Installation",
                            1000
                        )
                    ) {
                        logger.debug("Completed!!!");
                        break;
                    }
                    await new Promise((resolve) => setTimeout(resolve, 1000));
                }
                const completed = await eimRunner.findByText(
                    "Complete Installation"
                );
                expect(completed).to.not.be.false;
                expect(await completed.isDisplayed()).to.be.true;
            } catch (error) {
                logger.info("Failed to complete installation", error);
                throw error;
            }
        });

        it("Should offer to save installation configuration", async function () {
            this.timeout(1300000);

            try {
                await eimRunner.clickButton("Complete Installation");
                const completed = await eimRunner.findByText(
                    "Installation Complete!"
                );
                expect(await completed.isDisplayed()).to.be.true;
                const saveConfig = await eimRunner.findByText(
                    "Save Configuration"
                );
                expect(saveConfig).to.not.be.false;
                expect(await saveConfig.isDisplayed()).to.be.true;
                const exit = await eimRunner.findByText("Exit Installer");
                expect(exit).to.not.be.false;
            } catch (error) {
                logger.info("Failed to complete installation", error);
                throw error;
            }
        });
    });
}

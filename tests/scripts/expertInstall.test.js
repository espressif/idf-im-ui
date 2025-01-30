import os from "os";
import path from "path";
import { expect } from "chai";
import { describe, it, before, after, beforeEach, afterEach } from "mocha";
import { By, Key } from "selenium-webdriver";
import { EIMRunner } from "../classes/tauriRunner.class.js";
import logger from "../classes/logger.class.js";

let pathToEim;

if (process.env.EIM_GUI_PATH) {
    pathToEim = process.env.EIM_GUI_PATH;
} else {
    pathToEim =
        os.platform() !== "win32"
            ? path.resolve(os.homedir(), "eim-gui", "eim")
            : path.resolve(os.homedir(), "eim-gui", "eim.exe");
}

let eimRunner = "";

describe("EIM expert Installation", () => {
    before(async function () {
        this.timeout(30000);
        eimRunner = new EIMRunner(pathToEim);
        try {
            await eimRunner.launchEIM();
        } catch (err) {
            logger.info("Error starting EIM application");
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

        try {
            // Wait for the header to be present
            const header = await eimRunner.findByCSS("h1");
            const text = await header.getText();
            expect(text).to.equal("Welcome to ESP-IDF Installation Manager!");
        } catch (error) {
            logger.info("Failed to get Welcome header", error);
            throw error;
        }
    });

    it("Should show expert installation option", async function () {
        this.timeout(10000);

        try {
            await eimRunner.clickButton("Get Started");
            const expert = await eimRunner.findByDataId("expert-title");
            expect(await expert.getText()).to.equal("Expert Installation");
            expect(await expert.isDisplayed()).to.be.true;
        } catch (error) {
            logger.info("Failed to locate expert Installation", error);
            throw error;
        }
    });

    it("Should check prerequisites", async function () {
        this.timeout(10000);

        try {
            await eimRunner.clickButton("Start Expert Setup");
            const prerequisitesList = await eimRunner.findByDataId(
                "prerequisites-items-list"
            );
            expect(await prerequisitesList.getText()).to.not.include("❌");
            if (os.platform() === "win32") {
                const requisitesList = (
                    await prerequisitesList.getText()
                ).split("\n");
                expect(await prerequisitesList.getText()).to.not.be.empty;
                expect(requisitesList).to.include("git", "cmake", "ninja");
            } else {
                const requisitesList = (
                    await prerequisitesList.getText()
                ).split("✔");
                expect(await prerequisitesList.getText()).to.not.be.empty;
                expect(requisitesList).to.include(
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
                    "libusb-1.0-0"
                );
            }
        } catch (error) {
            logger.info("Failed to show installed prerequisites", error);
            throw error;
        }
    });

    it("Should check python installation", async function () {
        this.timeout(10000);
        try {
            await eimRunner.clickButton("Continue to Next Step");
            const result = await eimRunner.findByDataId("python-check-result");
            expect(await result.getText()).to.include(
                "Python Environment Ready"
            );
        } catch (error) {
            logger.info("Failed to show python Ready", error);
            throw error;
        }
    });

    it("Should show targets list", async function () {
        this.timeout(10000);
        try {
            await eimRunner.clickButton("Continue to Next Step");
            await new Promise((resolve) => setTimeout(resolve, 2000));
            const targetsList = await eimRunner.findByDataId("targets-grid");
            const targetsText = await targetsList.getText();
            expect(targetsText).to.include(
                "esp32",
                "esp32c2",
                "esp32c3",
                "esp32c5",
                "esp32c6",
                "esp32h2",
                "esp32p4",
                "esp32s2",
                "esp32s3"
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
        } catch (error) {
            logger.info("Failed to list of available targets", error);
            throw error;
        }
    });

    it("Should show IDF version list", async function () {
        this.timeout(10000);
        try {
            await eimRunner.clickButton("Continue with Selected Targets");
            await new Promise((resolve) => setTimeout(resolve, 2000));
            const IDFList = await eimRunner.findByDataId("versions-grid");
            const list = await IDFList.getText();
            expect(list).to.include("v5.4", "v5.3.2", "v5.1.5", "master");
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
            await eimRunner.clickButton("v5.2.3");
            const selectedVersions = await eimRunner.findByText(
                "Selected versions:"
            );
            expect(await selectedVersions.getText()).to.include(
                "Selected versions:",
                "v5.2.3"
            );
        } catch (error) {
            logger.info("Failed to list of available IDF versions", error);
            throw error;
        }
    });

    it("Should show IDF download mirrors", async function () {
        this.timeout(10000);
        try {
            await eimRunner.clickButton("Continue Installation");
            await new Promise((resolve) => setTimeout(resolve, 2000));
            const IDFMirrors = await eimRunner.findByDataId(
                "idf-mirror-radio-group"
            );
            let idfMList = await IDFMirrors.getText();
            expect(idfMList).to.include(
                "https://github.com",
                "https://jihulab.com/esp-mirror"
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
            await githubMirror.findElement(By.css("input")).click();
        } catch (error) {
            logger.info("Failed to list available IDF download mirrors", error);
            throw error;
        }
    });

    it("Should show tools download mirrors", async function () {
        this.timeout(10000);
        try {
            const toolsMirrors = await eimRunner.findByDataId(
                "tools-mirror-radio-group"
            );
            let toolsMList = await toolsMirrors.getText();
            expect(toolsMList).to.include(
                "https://github.com",
                "https://dl.espressif.com/github_assets",
                "https://dl.espressif.cn/github_assets"
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

            await githubMirror.findElement(By.css("input")).click();
        } catch (error) {
            logger.info("Failed to list available IDF download mirrors", error);
            throw error;
        }
    });

    it("Should show installation path", async function () {
        this.timeout(10000);
        try {
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
            const customFolder =
                os.platform() === "win32"
                    ? "C:\\.espressif"
                    : `${os.homedir()}/.espressif2`;
            await input.sendKeys(customFolder);
            expect(await input.getAttribute("value")).to.equal(customFolder);
        } catch (error) {
            logger.info("Failed to show option for installation path", error);
            throw error;
        }
    });

    it("Should show installation summary", async function () {
        this.timeout(10000);
        try {
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
            expect(await selectedVersions.getText()).to.equal("v5.2.3");
        } catch (error) {
            logger.info("Failed to show installation summary", error);
            throw error;
        }
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
                if (await eimRunner.findByText("Installation Failed", 1000)) {
                    logger.debug("failed!!!!");
                    break;
                }
                if (await eimRunner.findByText("Complete Installation", 1000)) {
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

    it("Should save installation configuration", async function () {
        this.timeout(1300000);

        try {
            await eimRunner.clickButton("Complete Installation");
            const completed = await eimRunner.findByText(
                "Installation Complete!"
            );
            expect(await completed.isDisplayed()).to.be.true;
            const saveConfig = await eimRunner.findByText("Save Configuration");
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

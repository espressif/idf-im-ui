import { expect } from "chai";
import { describe, it, before, after } from "mocha";
import { By, Key } from "selenium-webdriver";
import GUITestRunner from "../classes/GUITestRunner.class.js";
import { startTestProxy, stopTestProxy } from "../helpers/testProxy.js";
import {
  startGUIApp,
  stopGUIApp,
  registerGUIFailureHooks,
  expectWelcomePage,
  openInstallerSetup,
  waitForGUIInstallation,
} from "../helpers/guiTestHelpers.js";
import {
  IDFMIRRORS,
  TOOLSMIRRORS,
  PYPIMIRRORS,
  IDFAvailableVersions,
  availableTargets,
} from "../config.js";
import { getAvailableFeatures, getAvailableTools } from "../helper.js";
import { tGui } from "../helpers/i18n.js";
import logger from "../classes/logger.class.js";
import os from "os";

// This function executes the expert installation functionality of the EIM GUI
// Parameters are selected according to the arguments passed to the function, missing arguments sets the default value
export function runGUICustomInstallTest({
  id = 0,
  pathToEIM,
  installFolder,
  targetList,
  idfVersionList,
  toolsMirror,
  idfMirror,
  pypiMirror,
  testProxyMode = false,
  proxyBlockList = [],
}) {
  describe(`${id}- Run expert mode |`, () => {
    let eimRunner = null;
    let proxy = null;

    // The setup function should start the proxy server if enabled and start the EIM application GUI
    before(async function () {
      this.timeout(60000);
      eimRunner = new GUITestRunner(pathToEIM);
      if (testProxyMode) {
        proxy = await startTestProxy(testProxyMode, proxyBlockList);
      }
      await startGUIApp(eimRunner);
    });

    // Skip the next tests if the previous test failed and save a screenshot on failure
    registerGUIFailureHooks({ id, getRunner: () => eimRunner, screenshotTimeout: 10000 });
    
    // The tear down function should stop the EIM application GUI and proxy server if enabled
    after(async function () {
      this.timeout(5000);
      if (await stopGUIApp(eimRunner)) eimRunner = null;
      if (testProxyMode) {
        await stopTestProxy(proxy);
      }
    });

    it("01- Should show welcome page", async function () {
      this.timeout(45000);
      await expectWelcomePage(eimRunner, 10000);
    });

    it("02- Should show expert installation option", async function () {
      this.timeout(10000);      await openInstallerSetup(eimRunner);
      const custom = await eimRunner.findByDataId("custom-mode-card");
      expect(custom, "Expected option for custom installation").to.not.be.false;
      expect(await custom.getText()).to.include(
        tGui("basicInstaller.cards.custom.title"),
      );
      expect(
        await custom.isDisplayed(),
        "Expected option for simplified installation",
      ).to.be.true;
    });

    it("03- Should show targets list", async function () {
      this.timeout(45000);
      await eimRunner.clickByDataId("custom-mode-button");
      await new Promise((resolve) => setTimeout(resolve, 15000));
      const targetsList = await eimRunner.findByDataId("targets-grid", 20000);
      const targetsText = await targetsList.getText();
      for (let target of availableTargets) {
        expect(targetsText).to.include(target);
      }
      let targetAll = await eimRunner.findByDataId("select-all-targets");
      expect(
        await targetAll.findElement(By.css("Div")).getAttribute("class"),
      ).to.include("checked");
      let targetESP32 = await eimRunner.findByDataId("target-item-esp32");
      expect(await targetESP32.getAttribute("class")).to.not.include(
        "selected",
      );
      await eimRunner.clickByDataId("target-item-esp32");
      expect(
        await targetAll.findElement(By.css("Div")).getAttribute("class"),
      ).not.to.include("checked");
      expect(await targetESP32.getAttribute("class")).to.include("selected");
      await eimRunner.clickByDataId("target-item-esp32");
      expect(
        await targetAll.findElement(By.css("Div")).getAttribute("class"),
      ).to.include("checked");
      await eimRunner.clickByDataId("select-all-targets");

      for (let target of targetList) {
        if (target === "All") {
          await eimRunner.clickByDataId("select-all-targets");
        } else {
          await eimRunner.clickByDataId(`target-item-${target}`);
        }
        if (target === "All") {
          expect(
            await targetAll.findElement(By.css("Div")).getAttribute("class"),
          ).to.include("checked");
        } else {
          let selectedTarget = await eimRunner.findByDataId(
            `target-item-${target}`,
          );
          expect(await selectedTarget.getAttribute("class")).to.include(
            "selected",
          );
        }
      }
    });

    it("04- Should show IDF version list", async function () {
      this.timeout(15000);
      await eimRunner.clickByDataId("continue-targets-button");
      await new Promise((resolve) => setTimeout(resolve, 4000));

      const IDFListStable = await eimRunner.findByDataId(
        "stable-versions-section",
      );
      const IDFListStableText = await IDFListStable.getText();
      for (let version of IDFAvailableVersions.stable) {
        expect(IDFListStableText).to.include(version);
      }
      const IDFListPrerelease = await eimRunner.findByDataId(
        "prerelease-versions-section",
      );
      const IDFListPrereleaseText = await IDFListPrerelease.getText();
      for (let version of IDFAvailableVersions.prerelease) {
        expect(IDFListPrereleaseText).to.include(version);
      }
      const IDFListDevelopment = await eimRunner.findByDataId(
        "development-versions-section",
      );
      const IDFListDevelopmentText = await IDFListDevelopment.getText();
      for (let version of IDFAvailableVersions.development) {
        expect(IDFListDevelopmentText).to.include(version);
      }
      let IDFMaster = await eimRunner.findByDataId("version-item-master");
      expect(await IDFMaster.getAttribute("class")).to.not.include("selected");
      await eimRunner.driver.executeScript("arguments[0].click();", IDFMaster);
      expect(await IDFMaster.getAttribute("class")).to.include("selected");
      const selectedMaster = await eimRunner.findByDataId(
        "selected-tag-master",
      );
      let closeButton = await selectedMaster.findElement(By.css("button"));
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        closeButton,
      );
      expect(await IDFMaster.getAttribute("class")).to.not.include("selected");
      for (let version of idfVersionList) {
        await eimRunner.clickByDataId(`version-item-${version}`);
      }
      const selectedVersions = await eimRunner.findByDataId("selected-tags");
      const selectedVersionsText = await selectedVersions.getText();
      for (let version of idfVersionList) {
        expect(selectedVersionsText).to.include(version);
      }
    });

    it("05- Should show IDF download mirrors", async function () {
      this.timeout(15000);
      await eimRunner.clickByDataId("continue-installation-button");
      await new Promise((resolve) => setTimeout(resolve, 2000));
      const IDFMirrorsList = await eimRunner.findByDataId("idf-mirror-section");
      let IDFMirrorsListText = await IDFMirrorsList.getText();
      for (let mirror of Object.values(IDFMIRRORS)) {
        expect(IDFMirrorsListText).to.include(mirror);
      }

      let githubMirror = await eimRunner.findByDataId(
        "idf-mirror-option-https://github.com",
      );
      let espressifCnMirror = await eimRunner.findByDataId(
        "idf-mirror-option-https://git.espressif.com.cn",
      );

      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        githubMirror,
      );
      expect(await githubMirror.getAttribute("class")).to.include("selected");
      expect(await espressifCnMirror.getAttribute("class")).to.not.include(
        "selected",
      );
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        espressifCnMirror,
      );
      expect(await githubMirror.getAttribute("class")).to.not.include(
        "selected",
      );
      expect(await espressifCnMirror.getAttribute("class")).to.include("selected");

      idfMirror === "github" &&
        (await eimRunner.driver.executeScript(
          "arguments[0].click();",
          githubMirror,
        ));
      idfMirror === "gitmirror" &&
        (await eimRunner.driver.executeScript(
          "arguments[0].click();",
          espressifCnMirror,
        ));
    });

    it("06- Should show tools download mirrors", async function () {
      this.timeout(10000);
      const toolsMirrorsList = await eimRunner.findByDataId(
        "tools-mirror-section",
      );
      let toolsMirrorsListText = await toolsMirrorsList.getText();
      for (let mirror of Object.values(TOOLSMIRRORS)) {
        expect(toolsMirrorsListText).to.include(mirror);
      }
      let githubMirror = await eimRunner.findByDataId(
        "tools-mirror-option-https://github.com",
      );
      let espressifComMirror = await eimRunner.findByDataId(
        "tools-mirror-option-https://dl.espressif.com/github_assets",
      );
      let espressifCnMirror = await eimRunner.findByDataId(
        "tools-mirror-option-https://dl.espressif.cn/github_assets",
      );
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        githubMirror,
      );
      expect(await githubMirror.getAttribute("class")).to.include("selected");
      expect(await espressifComMirror.getAttribute("class")).to.not.include(
        "selected",
      );
      expect(await espressifCnMirror.getAttribute("class")).to.not.include(
        "selected",
      );
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        espressifComMirror,
      );
      expect(await githubMirror.getAttribute("class")).to.not.include(
        "selected",
      );
      expect(await espressifComMirror.getAttribute("class")).to.include(
        "selected",
      );
      expect(await espressifCnMirror.getAttribute("class")).to.not.include(
        "selected",
      );
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        espressifCnMirror,
      );
      expect(await githubMirror.getAttribute("class")).to.not.include(
        "selected",
      );
      expect(await espressifComMirror.getAttribute("class")).to.not.include(
        "selected",
      );
      expect(await espressifCnMirror.getAttribute("class")).to.include(
        "selected",
      );

      toolsMirror === "github" &&
        (await eimRunner.driver.executeScript(
          "arguments[0].click();",
          githubMirror,
        ));
      toolsMirror === "dl_com" &&
        (await eimRunner.driver.executeScript(
          "arguments[0].click();",
          espressifComMirror,
        ));
      toolsMirror === "dl_cn" &&
        (await eimRunner.driver.executeScript(
          "arguments[0].click();",
          espressifCnMirror,
        ));
    });

    it("07- Should show PyPI download mirrors", async function () {
      this.timeout(10000);
      const pypiMirrorsList = await eimRunner.findByDataId(
        "pypi-mirror-section",
      );
      let pypiMirrorsListText = await pypiMirrorsList.getText();
      for (let mirror of Object.values(PYPIMIRRORS)) {
        expect(pypiMirrorsListText).to.include(mirror);
      }
      let officialMirror = await eimRunner.findByDataId(
        "pypi-mirror-option-https://pypi.org/simple",
      );
      let aliyunMirror = await eimRunner.findByDataId(
        "pypi-mirror-option-https://mirrors.aliyun.com/pypi/simple",
      );
      let tsinghuaMirror = await eimRunner.findByDataId(
        "pypi-mirror-option-https://pypi.tuna.tsinghua.edu.cn/simple",
      );
      let ustcMirror = await eimRunner.findByDataId(
        "pypi-mirror-option-https://pypi.mirrors.ustc.edu.cn/simple",
      );
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        officialMirror,
      );
      expect(await officialMirror.getAttribute("class")).to.include("selected");
      expect(await aliyunMirror.getAttribute("class")).to.not.include(
        "selected",
      );
      expect(await tsinghuaMirror.getAttribute("class")).to.not.include(
        "selected",
      );
      expect(await ustcMirror.getAttribute("class")).to.not.include("selected");

      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        aliyunMirror,
      );
      expect(await officialMirror.getAttribute("class")).to.not.include(
        "selected",
      );
      expect(await aliyunMirror.getAttribute("class")).to.include("selected");
      expect(await tsinghuaMirror.getAttribute("class")).to.not.include(
        "selected",
      );
      expect(await ustcMirror.getAttribute("class")).to.not.include("selected");

      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        tsinghuaMirror,
      );
      expect(await officialMirror.getAttribute("class")).to.not.include(
        "selected",
      );
      expect(await aliyunMirror.getAttribute("class")).to.not.include(
        "selected",
      );
      expect(await tsinghuaMirror.getAttribute("class")).to.include("selected");
      expect(await ustcMirror.getAttribute("class")).to.not.include("selected");

      await eimRunner.driver.executeScript("arguments[0].click();", ustcMirror);
      expect(await officialMirror.getAttribute("class")).to.not.include(
        "selected",
      );
      expect(await aliyunMirror.getAttribute("class")).to.not.include(
        "selected",
      );
      expect(await tsinghuaMirror.getAttribute("class")).to.not.include(
        "selected",
      );
      expect(await ustcMirror.getAttribute("class")).to.include("selected");

      await eimRunner.clickByDataId(
        `pypi-mirror-option-${PYPIMIRRORS[pypiMirror]}`,
      );
    });

    it("08- Should show list of optional features", async function () {
      this.timeout(10000);
      await eimRunner.clickByDataId("continue-mirrors-button");
      await new Promise((resolve) => setTimeout(resolve, 4000));

      for (let version of idfVersionList) {
        if (idfVersionList.length > 1) {
          await eimRunner.clickByDataId(`version-tab-button-${version}`);
        }

        const requiredFeaturesList =
          await eimRunner.findByDataId("required-group");
        const requiredFeaturesListText = await requiredFeaturesList.getText();
        expect(
          requiredFeaturesListText,
          "Core feature not added to the required features",
        ).to.include("core");

        const optionalFeaturesList =
          await eimRunner.findByDataId("optional-group");
        const optionalFeaturesListText = await optionalFeaturesList.getText();
        const expectedFeaturesAll = await getAvailableFeatures(version);
        const expectedFeaturesOptional = expectedFeaturesAll.filter(
          (feature) => feature !== "core",
        );
        for (let feature of expectedFeaturesOptional) {
          expect(
            optionalFeaturesListText,
            `Feature ${feature} not listed in the optional features`,
          ).to.include(feature);
        }
      }
    });

    it("09- Should show list of Tools", async function () {
      this.timeout(10000);
      await eimRunner.clickByDataId("continue-features-button");
      await new Promise((resolve) => setTimeout(resolve, 4000));

      for (let version of idfVersionList) {
        if (idfVersionList.length > 1) {
          await eimRunner.clickByDataId(`version-tab-button-${version}`);
        }

        const completeToolsList =
          await eimRunner.findByDataId("tools-sections");
        const completeToolsListText = await completeToolsList.getText();
        const expectedToolsAll = await getAvailableTools(version, targetList);

        for (let tool of expectedToolsAll) {
          expect(
            completeToolsListText,
            `Tool ${tool} not listed in the tools list`,
          ).to.include(tool);
        }
      }
    });

    it("10- Should show installation path", async function () {
      this.timeout(10000);
      await eimRunner.clickByDataId("continue-tools-button");
      await new Promise((resolve) => setTimeout(resolve, 2000));
      const installPath = await eimRunner.findByDataId("path-info-title");
      expect(await installPath.getText()).to.equal(
        tGui("installationPathSelect.info.title"),
      );
      expect(await installPath.isDisplayed()).to.be.true;
      const pathInput = await eimRunner.findByDataId("installation-path-input");
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

    it("11- Should show installation summary", async function () {
      this.timeout(10000);
      await eimRunner.clickByDataId("continue-path-button");
      await new Promise((resolve) => setTimeout(resolve, 2000));
      const versionSummary = await eimRunner.findByDataId("versions-info");
      expect(await versionSummary.getText()).to.include(
        tGui("installationProgress.normalMode.title"),
      );
      const selectedVersions = await eimRunner.findByDataId("version-chips");
      const selectedVersionsText = await selectedVersions.getText();
      for (let idfVersion of idfVersionList) {
        expect(selectedVersionsText).to.include(idfVersion);
      }
    });

    it("12- Should install IDF using expert setup", async function () {
      this.timeout(2730000);

      try {
        await eimRunner.clickByDataId("start-installation-button");
        await new Promise((resolve) => setTimeout(resolve, 2000));
        const installing = await eimRunner.findByDataId("installation-title");
        expect(await installing.isDisplayed()).to.be.true;
        expect(await installing.getText()).to.equal(
          tGui("installationProgress.title.installation"),
        );
        await waitForGUIInstallation(eimRunner, {
          failureIds: ["error-message"],
          completeId: "complete-installation-button-footer",
        });
        const completed = await eimRunner.findByDataId(
          "complete-installation-button-footer",
        );
        expect(completed).to.not.be.false;
        expect(await completed.isDisplayed()).to.be.true;
      } catch (error) {
        logger.info("Failed to complete installation", error);
        throw error;
      }
    });

    it("13- Should offer to save installation configuration", async function () {
      this.timeout(15000);

      try {
        await eimRunner.clickByDataId("complete-installation-button-footer");
        await new Promise((resolve) => setTimeout(resolve, 5000));
        const completed = await eimRunner.findByDataId("completion-result");
        expect(await completed.isDisplayed()).to.be.true;
        expect(await completed.getText()).to.include(tGui("complete.title"));
        const saveConfig = await eimRunner.findByDataId("save-config-button");
        expect(saveConfig).to.not.be.false;
        expect(await saveConfig.isDisplayed()).to.be.true;
        const exit = await eimRunner.findByDataId("exit-button");
        expect(exit).to.not.be.false;
      } catch (error) {
        logger.info("Failed to complete installation", error);
        throw error;
      }
    });
  });
}

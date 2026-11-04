import { expect } from "chai";
import { describe, it } from "mocha";
import logger from "../classes/logger.class.js";
import {
  registerGUIAppLifecycle,
  registerGUIFailureHooks,
  expectWelcomePage,
  openManageInstallations,
} from "../helpers/guiTestHelpers.js";
import { By } from "selenium-webdriver";

// This function verifies the presence of the installed IDF versions in the dashboard
export function runGUIAfterInstallTest({ id = 0, pathToEIM, idfList }) {
  
  describe(`${id}- EIM GUI After Install |`, () => {
    let eimRunner = null;
    let totalInstallations = 0;

    registerGUIAppLifecycle({
      pathToEIM,
      getRunner: () => eimRunner,
      setRunner: (runner) => (eimRunner = runner),
    });

    registerGUIFailureHooks({ id, getRunner: () => eimRunner });

    it("1- Should show welcome page", async function () {
      this.timeout(45000);
      await expectWelcomePage(eimRunner, 10000);
    });

    it("2- Should show option to manage installations", async function () {
      this.timeout(10000);
      totalInstallations = await openManageInstallations(eimRunner);
    });

    it("3- Should show dashboard with installations", async function () {
      this.timeout(10000);
      await new Promise((resolve) => setTimeout(resolve, 5000));
      const cards = await eimRunner.findMultipleByClass("n-card");
      expect(cards.length, "Expected matching number of cards").to.be.equal(
        totalInstallations
      );
      let versionsList = [];
      for (let card of cards) {
        const versionElement = await card.findElement(
          By.className("version-info")
        );
        const versionText = await versionElement.getText();
        versionsList.push(versionText);
      }
      logger.debug(`Installed versions: ${versionsList}`);
      for (let idfVersion of idfList) {
        expect(
          versionsList.includes(idfVersion),
          `Expected dashboard card to be shown for version ${idfVersion} `
        ).to.be.true;
      }
    });
  });
}

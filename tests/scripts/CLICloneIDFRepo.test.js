/**
 * Clone-only helper: clones the ESP-IDF git repository to a given path.
 * Used by the existing-git-clone test flow before running EIM install -p <path>.
 */

import { expect } from "chai";
import { describe, it, before } from "mocha";
import logger from "../classes/logger.class.js";
import path from "path";
import fs from "fs";
import { execSync } from "child_process";

/**
 * Runs a describe block that clones the IDF repo to the specified path.
 * Does not start any EIM or verification; caller runs custom install and verification after.
 *
 * @param {Object} options
 * @param {string} [options.id=0] - Sub-id for the describe title
 * @param {string} options.path - Full path where the repo will be cloned (will be created/cleaned)
 * @param {string} [options.gitRepoUrl] - Git repo URL (default: esp-idf upstream)
 * @param {string} [options.gitRepoBranch] - Branch or tag to clone (default: v5.5.3)
 */
export function runCLIClonedIDFRepo({
  id = 0,
  path: clonePath,
  gitRepoUrl = "https://github.com/espressif/esp-idf.git",
  gitRepoBranch = "v5.5.3",
}) {
  describe(`${id}- Clone IDF repo |`, function () {
    before(async function () {
      this.timeout(120000);
      logger.info(`Cloning IDF repository: ${gitRepoUrl} branch ${gitRepoBranch} -> ${clonePath}`);
      fs.rmSync(clonePath, { recursive: true, force: true });
      fs.mkdirSync(path.dirname(clonePath), { recursive: true });
      execSync(`git clone --branch ${gitRepoBranch} "${gitRepoUrl}" "${clonePath}"`, {
        stdio: "inherit",
        timeout: 120000,
      });
      logger.info("IDF repository cloned successfully");
    });

    it("1- Clone folder should exist and contain esp-idf files", function () {
      expect(fs.existsSync(clonePath), `Clone path should exist: ${clonePath}`).to.be.true;
      const hasIdfFiles =
        fs.existsSync(path.join(clonePath, "tools")) &&
        (fs.existsSync(path.join(clonePath, "components")) || fs.existsSync(path.join(clonePath, "requirements.txt")));
      expect(hasIdfFiles, "Clone should contain IDF structure (e.g. tools/)").to.be.true;
    });
  });
}

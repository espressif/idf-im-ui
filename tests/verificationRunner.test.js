import { runInstallVerification } from "./scripts/installationVerification.test.js";
import os from "os";
import path from "path";

const installFolder =
  os.platform() !== "win32" ? path.join(os.homedir(), `.espressif`) : `C:\\esp`;

const idfList = ["v5.4.2"];

const targetList = ["All"];

runInstallVerification({ installFolder, idfList, targetList });

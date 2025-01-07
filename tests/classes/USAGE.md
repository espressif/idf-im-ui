# Using the class on node terminal

Import the system modules:

const {default:os} = await import("os");
const {default:path} = await import("path")
const { expect } = await import("chai");
const { By, Key } = await import("selenium-webdriver");
const application = path.resolve(os.homedir(), "eim-gui", "eim.exe");
const {EIMRunner}= await import("./classes/tauriRunner.class.js");
let eimRunner = new EIMRunner(application);
eimRunner.launchEIM();

const {default:os} = await import("os");
const {default:path} = await import("path")
const { expect } = await import("chai");
const { By, Key } = await import("selenium-webdriver");
const application = path.resolve(os.homedir(), "eim-gui", "eim");
const {EIMRunner}= await import("./classes/tauriRunner.class.js");
let eimRunner = new EIMRunner(application);
eimRunner.launchEIM();

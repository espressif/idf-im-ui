import os from "os";
import path from "path";
import logger from "./logger.class.js";
import { spawn } from "child_process";
import { Builder, By, Capabilities, until } from "selenium-webdriver";

export class EIMRunner {
    constructor(application) {
        logger.debug(`Starting EIM from path ${application}`);
        this.application = application;
        this.capabilities = new Capabilities();

        this.capabilities.set("tauri:options", {
            application,
            webviewOptions: {},
        });
        this.capabilities.setBrowserName("wry");
    }

    async launchEIM() {
        logger.info("Lauching Tauri Driver");
        try {
            this.tauriDriver = spawn(
                path.resolve(os.homedir(), ".cargo", "bin", "tauri-driver"),
                [],
                { stdio: [null, process.stdout, process.stderr] }
            );
        } catch (error) {
            logger.info("Error launching Tauri driver:", error);
            throw error;
        }

        // Wait for tauri-driver to start
        await new Promise((resolve) => setTimeout(resolve, 1000));

        try {
            this.driver = await new Builder()
                .withCapabilities(this.capabilities)
                .usingServer("http://127.0.0.1:4444")
                .build();
        } catch (error) {
            logger.info("Error building driver:", error);
            throw error;
        }
    }

    async closeEIM() {
        if (this.driver) {
            try {
                await this.driver.quit();
            } catch (error) {
                logger.info("Error quitting driver:", error);
            }
        }
        try {
            if (this.tauriDriver) {
                this.tauriDriver.kill();
            }
        } catch (error) {
            logger.info("Error closing Tauri driver:", error);
        }
    }

    async findById(id, timeout = 5000) {
        try {
            const element = await this.driver.wait(
                until.elementLocated(By.id(id)),
                timeout,
                `Element with id ${id} not found`
            );
            logger.debug(`Selected element ${element}`);
            return element;
        } catch (error) {
            logger.debug(`Error during selection: ${error}`);
            return false;
        }
    }

    async findByClass(className, timeout = 5000) {
        try {
            const element = await this.driver.wait(
                until.elementLocated(By.className(className)),
                timeout,
                `Element with class ${className} not found`
            );
            logger.debug(`Selected element ${element}`);
            return element;
        } catch (error) {
            logger.debug(`Error during selection: ${error}`);
            return false;
        }
    }

    async findByCSS(cssAttribute, timeout = 5000) {
        try {
            const element = await this.driver.wait(
                until.elementLocated(By.css(cssAttribute)),
                timeout,
                `Element with attribute ${cssAttribute} not found`
            );
            logger.debug(`Selected element ${element}`);
            return element;
        } catch (error) {
            logger.debug(`Error during selection: ${error}`);
            return false;
        }
    }

    async findByDataId(dataId, timeout = 5000) {
        try {
            const element = await this.driver.wait(
                until.elementLocated(By.css(`[data-id="${dataId}"]`)),
                timeout,
                `Element with test ID ${dataId} not found`
            );
            logger.debug(`Selected element ${element}`);
            return element;
        } catch (error) {
            logger.debug(`Error during selection: ${error}`);
            return false;
        }
    }

    async findByText(text, timeout = 5000) {
        try {
            const element = await this.driver.wait(
                until.elementLocated(
                    By.xpath(`//*[contains(text(), '${text}')]`)
                ),
                timeout,
                `Element containing text "${text}" not found`
            );
            logger.debug(`Selected element ${element}`);
            return element;
        } catch (error) {
            logger.debug(`Error during selection: ${error}`);
            return false;
        }
    }

    async clickButton(text, timeout = 5000) {
        try {
            const button = await this.driver.wait(
                until.elementLocated(
                    By.xpath(`//*[contains(text(), '${text}')]`)
                ),
                timeout,
                `Button with text "${text}" not found`
            );
            logger.debug(`Selected button ${button}`);
            await button.click();
        } catch (error) {
            logger.debug(`Error during selection: ${error}`);
            return false;
        }
    }
}

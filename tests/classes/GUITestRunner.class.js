/**
 * This class is used to run a GUI application for the GUI tests.
 * The GUI application is launched using selenium webdriver tauri-driver.
 * Several methods are provided to allow better control of the GUI application.
 * Alternatively pure selenium webdriver commands can be used to control the GUI application.
 * 
 * The GUI application is started and keep running until the stop process is called, or any error occurs.
 * 
 */


import os from "os";
import path from "path";
import fs from "fs";
import logger from "./logger.class.js";
import { spawn } from "child_process";
import { Builder, By, Capabilities, until } from "selenium-webdriver";

class GUITestRunner {
  constructor(application) {
    logger.debug(`Starting EIM from path ${application}`);
    this.application = application;
    this.capabilities = new Capabilities();

    this.capabilities.set("tauri:options", {
      application,
    });
    this.capabilities.setBrowserName("wry");
  }

  // Function to launch the GUI application
  async start() {
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
    await new Promise((resolve) => setTimeout(resolve, 3000));

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

  // Function to stop the GUI application
  async stop() {
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

  // method to find an element by its HTML id attribute
  async findById(id, timeout = 5000) {
    try {
      const element = await this.driver.wait(
        until.elementLocated(By.id(id)),
        timeout,
        `Element with id ${id} not found`
      );
      logger.debug(`Selected html element ${await element.getTagName()}`);
      return element;
    } catch (error) {
      logger.debug(`Error during selection: ${error}`);
      return false;
    }
  }

  // method to find an element by its HTML class attribute
  // if more than one element exists it will only capture the first one
  async findByClass(className, timeout = 5000) {
    try {
      const element = await this.driver.wait(
        until.elementLocated(By.className(className)),
        timeout,
        `Element with class ${className} not found`
      );
      logger.debug(`Selected html element ${await element.getTagName()}`);
      return element;
    } catch (error) {
      logger.debug(`Error during selection: ${error}`);
      return false;
    }
  }

  // method to find multiple elements by their HTML class attribute
  // If only one element exists it will still return an array with a single element
  async findMultipleByClass(className, timeout = 5000) {
    try {
      const elements = await this.driver.wait(
        until.elementsLocated(By.className(className)),
        timeout,
        `Elements with class ${className} not found`
      );
      logger.debug(`Selected html elements matching class ${className}`);
      return elements;
    } catch (error) {
      logger.debug(`Error during selection: ${error}`);
      return false;
    }
  }

  // method to find an element by its HTML CSS attribute
  async findByCSS(cssAttribute, timeout = 5000) {
    try {
      const element = await this.driver.wait(
        until.elementLocated(By.css(cssAttribute)),
        timeout,
        `Element with attribute ${cssAttribute} not found`
      );
      logger.debug(`Selected html element ${await element.getTagName()}`);
      return element;
    } catch (error) {
      logger.debug(`Error during selection: ${error}`);
      return false;
    }
  }

  // method to find an element by its HTML data-id attribute
  // Data-id names were conveniently added to several objects to allow easier identification in the GUI tests
  async findByDataId(dataId, timeout = 5000) {
    try {
      const element = await this.driver.wait(
        until.elementLocated(By.css(`[data-id="${dataId}"]`)),
        timeout,
        `Element with test ID ${dataId} not found`
      );
      logger.debug(`Selected html element ${await element.getTagName()}`);
      return element;
    } catch (error) {
      logger.debug(`Error during selection: ${error}`);
      return false;
    }
  }

  // method to find an element by its text content
  async findByText(text, timeout = 5000) {
    try {
      const element = await this.driver.wait(
        until.elementLocated(By.xpath(`//*[contains(text(), '${text}')]`)),
        timeout,
        `Element containing text "${text}" not found`
      );
      logger.debug(`Selected html element ${await element.getTagName()}`);
      return element;
    } catch (error) {
      logger.debug(`Error during selection: ${error}`);
      return false;
    }
  }

  // method to find an element by its relation to another element
  // Use this method to select an element upstream or downstream to a known element. Relation and tag refers to the element you want to find.
  // text is the text content of the reference element.
  async findByRelation(relation, tag, text, timeout = 5000) {
    try {
      const element = await this.driver.wait(
        until.elementLocated(
          By.xpath(`//*[contains(text(), '${text}')]/${relation}::${tag}`)
        ),
        timeout,
        `Element ${tag} containing text "${text}" not found`
      );
      logger.debug(`Selected html element ${await element.getTagName()}`);
      return element;
    } catch (error) {
      logger.debug(`Error during selection: ${error}`);
      return false;
    }
  }

  // method to click a button by its text content
  async clickButton(text, timeout = 5000) {
    try {
      const button = await this.driver.wait(
        until.elementLocated(
          By.xpath(`//*[contains(text(), '${text}')]/ancestor-or-self::button`)
        ),
        timeout,
        `Button with text "${text}" not found`
      );
      logger.debug(
        `Selected button element with text ${await button.getText()}`
      );
      await this.driver.executeScript("arguments[0].click();", button);
      return true;
    } catch (error) {
      logger.debug(`Error during selection: ${error}`);
      return false;
    }
  }

  // method to click any element by its text content
  async clickElement(text, timeout = 5000) {
    try {
      const element = await this.driver.wait(
        until.elementLocated(By.xpath(`//*[contains(text(), '${text}')]`)),
        timeout,
        `Element with text "${text}" not found`
      );
      logger.debug(
        `Selected element ${await element.getTagName()} with text ${await element.getText()}`
      );
      await this.driver.executeScript("arguments[0].click();", element);
    } catch (error) {
      logger.debug(`Error during selection: ${error}`);
      return false;
    }
  }

  // Method to take a screenshot of the current GUI state
  // Thsi is mostly used for debug
  async takeScreenshot(filename) {
    try {
      const screenshot = await this.driver.takeScreenshot();
      fs.writeFileSync(filename, screenshot, "base64");
    } catch (error) {
      logger.info("Error taking screenshot:", error);
      throw error;
    }
  }
}

export default GUITestRunner;

import TestProxy from "../classes/TestProxy.class.js";
import logger from "../classes/logger.class.js";

// Returns the proxy (even if it failed to start) or null if it could not be created
async function startTestProxy(mode, blockedDomains) {
  let proxy = null;
  try {
    proxy = new TestProxy({ mode, blockedDomains });
    await proxy.start();
  } catch (error) {
    logger.info("Error to start proxy server");
    logger.debug(`Error: ${error}`);
  }
  return proxy;
}

async function stopTestProxy(proxy) {
  try {
    await proxy.stop();
  } catch (error) {
    logger.info("Error stopping proxy server");
    logger.info(`Error: ${error}`);
  }
}

export { startTestProxy, stopTestProxy };

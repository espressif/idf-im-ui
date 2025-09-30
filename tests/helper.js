import logger from "./classes/logger.class.js";
import { IDFDefaultVersion, pkgName } from "./config.js";
import { Readable } from "stream";
import { finished } from "stream/promises";
import os from "os";
import path from "path";

// Base url for offline archive files
const offlineBaseUrl = "https://dl.espressif.com/dl/eim/archive_";

// function to get the tag for the operating system to use when reading tools.json
function getPlatformKey() {
  const arch = os.arch();
  const platform = os.platform();

  if (platform === "linux") {
    if (arch === "x64") return "linux-amd64";
    if (arch === "arm64") return "linux-arm64";
    if (arch === "arm") return "linux-armhf";
    if (arch === "ia32") return "linux-i686";
  }
  if (platform === "darwin") {
    if (arch === "x64") return "macos";
    if (arch === "arm64") return "macos-arm64";
  }
  if (platform === "win32") {
    if (arch === "x64") return "win64";
    if (arch === "ia32") return "win32";
  }
  return null;
}

// function to get the OS name matching strings from GUI
function getOSName() {
  const platform = os.platform();
  if (platform === "linux") {
    return "linux";
  }
  if (platform === "darwin") {
    return "macOS";
  }
  if (platform === "win32") {
    return "windows";
  }
  return "Unknown OS";
}

// function to get the platform architecture matching strings from GUI
function getArchitecture() {
  const arch = os.arch();
  if (arch === "x64") {
    return "x86_64";
  }
  if (arch === "arm64") {
    return "aarch64";
  }
  return "Unknown Architecture";
}

// function to download the offline archive for a given IDF version and provide
// the path to the downloaded file
const downloadOfflineArchive = async ({
  idfVersion = IDFDefaultVersion,
  packageName = pkgName,
}) => {
  const archiveUrl = `${offlineBaseUrl}${idfVersion}_${packageName}.zst`;
  const pathToOfflineArchive = path.resolve(
    process.cwd(),
    `offlineArchive_${idfVersion}.zst`
  );
  logger.info(`Downloading offline archive from ${archiveUrl}...`);
  try {
    const res = await fetch(archiveUrl);
    if (res.ok) {
      const fileSize = parseInt(res.headers.get("content-length") || "0");
      let downloadedBytes = 0;

      // Create write stream
      const fileStream = fs.createWriteStream(pathToOfflineArchive);

      // Create readable stream from response body
      const readStream = Readable.fromWeb(res.body);

      // Log progress on data chunks
      readStream.on("data", (chunk) => {
        downloadedBytes += chunk.length;
        const progress = (downloadedBytes / fileSize) * 100;

        // Log every 5% progress
        if (progress % 5 < (chunk.length / fileSize) * 100) {
          logger.info(`Download progress: ${progress.toFixed(1)}%`);
        }
      });
      // Pipe response to file and wait for completion
      await finished(readStream.pipe(fileStream));
      logger.info(`Offline archive downloaded to ${pathToOfflineArchive}`);
      return pathToOfflineArchive;
    } else {
      throw new Error(`Failed to download archive: ${res.statusText}`);
    }
  } catch (error) {
    logger.error(`Error downloading offline archive: ${error.message}`);
    return false;
  }
};

export { getPlatformKey, getOSName, getArchitecture, downloadOfflineArchive };

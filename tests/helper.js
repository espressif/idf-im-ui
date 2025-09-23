import logger from "./classes/logger.class.js";
import os from "os";
import path from "path";

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

export { getPlatformKey, getOSName, getArchitecture };

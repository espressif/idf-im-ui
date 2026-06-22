/**
 * Shared state + helpers for the "advanced / optional" path options that
 * are exposed alongside the main installation-path field:
 *
 *   - cleanup flag: delete downloaded archives after install
 *   - custom tool folder names: rename the tool download / install folders
 *
 * Originally added to `InstallationPathSelect.vue` (EIM-863) and reused
 * by `OfflineInstaller.vue` so the offline flow has the same opt-in
 * controls. Centralised here to keep the two views in sync (per the DRY
 * rule in AGENTS.md): if a new option is added or an i18n key moves,
 * both surfaces pick it up automatically.
 *
 * The module is intentionally framework-agnostic: it exports plain
 * functions that take the reactive state as a parameter, so it works
 * with the Options API `data()` shape used in this repo (just assign
 * the returned object to `data()`) and with a `<script setup>` ref if
 * it gets adopted later.
 */
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { path as tauriPath } from "@tauri-apps/api";

export const TOOL_DOWNLOAD_FOLDER_DEFAULT = "dist";
export const TOOL_INSTALL_FOLDER_DEFAULT = "tools";

/**
 * Build the initial reactive state for the advanced options. Returned
 * as a plain object so it can be spread into a component's `data()`.
 */
export function createAdvancedPathOptionsState() {
  return {
    cleanup: false,
    customToolFolders: false,
    toolDownloadFolderName: TOOL_DOWNLOAD_FOLDER_DEFAULT,
    toolInstallFolderName: TOOL_INSTALL_FOLDER_DEFAULT,
  };
}

/**
 * Hydrate the state from the persisted settings. Should be called from
 * `mounted()` so the UI reflects whatever the user previously chose.
 *
 * The backend's `Settings::default()` stores the tool folder names as
 * full paths (e.g. `/home/runner/.espressif/dist`), while the wizard and
 * `browseToolFolder` persist just the basename. Normalise to the basename
 * so the input always shows the final folder segment and the
 * "renamed away from default" check works regardless of which shape
 * the persisted value has.
 */
export async function loadAdvancedPathOptions(state) {
  const downloadFolder = await invoke("get_tool_download_folder_name");
  const installFolder = await invoke("get_tool_install_folder_name");

  const downloadBase =
    downloadFolder && downloadFolder.length > 0
      ? await tauriPath.basename(downloadFolder)
      : "";
  const installBase =
    installFolder && installFolder.length > 0
      ? await tauriPath.basename(installFolder)
      : "";

  state.toolDownloadFolderName =
    downloadBase || TOOL_DOWNLOAD_FOLDER_DEFAULT;
  state.toolInstallFolderName = installBase || TOOL_INSTALL_FOLDER_DEFAULT;

  // If either folder was renamed away from its default, the user opted
  // into custom folders on a previous run — show the checkbox as
  // checked so they are not surprised by hidden fields.
  state.customToolFolders =
    (!!downloadBase && downloadBase !== TOOL_DOWNLOAD_FOLDER_DEFAULT) ||
    (!!installBase && installBase !== TOOL_INSTALL_FOLDER_DEFAULT);

  state.cleanup = await invoke("get_cleanup");
}

/**
 * Persist cleanup immediately so the choice survives even if the user
 * navigates away without pressing Continue / Start.
 */
export async function onCleanupChange(state, checked) {
  state.cleanup = checked;
  await invoke("set_cleanup", { cleanup: checked });
}

/**
 * When the user turns the custom-folder option off, reset the folder
 * names to their defaults so an edited-then-abandoned value is not
 * silently kept in component state.
 */
export function onCustomFoldersToggle(state, checked) {
  state.customToolFolders = checked;
  if (!checked) {
    state.toolDownloadFolderName = TOOL_DOWNLOAD_FOLDER_DEFAULT;
    state.toolInstallFolderName = TOOL_INSTALL_FOLDER_DEFAULT;
  }
}

/**
 * Open a directory picker for a tool folder. These settings are folder
 * NAMES (appended under the install path), so we keep only the final
 * path component. If a future backend accepts a full path, swap
 * `tauriPath.basename(selected)` for `selected`.
 */
export async function browseToolFolder(state, which) {
  if (!state.customToolFolders) return;
  const selected = await open({
    directory: true,
    multiple: false,
  });
  if (!selected) return;
  const name = await tauriPath.basename(selected);
  if (which === "download") {
    state.toolDownloadFolderName = name;
  } else {
    state.toolInstallFolderName = name;
  }
}

/**
 * Persist the current state to the backend. Only the custom-folder
 * names are pushed when the user actually opted in, so leaving the
 * option off never overwrites a previously-saved value with the
 * default.
 */
export async function persistAdvancedPathOptions(state) {
  if (state.customToolFolders) {
    await invoke("set_tool_download_folder_name", {
      name: state.toolDownloadFolderName || TOOL_DOWNLOAD_FOLDER_DEFAULT,
    });
    await invoke("set_tool_install_folder_name", {
      name: state.toolInstallFolderName || TOOL_INSTALL_FOLDER_DEFAULT,
    });
  }
  await invoke("set_cleanup", { cleanup: state.cleanup });
}

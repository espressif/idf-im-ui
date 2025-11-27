import { defineStore } from "pinia";
import { invoke } from '@tauri-apps/api/core'

export const useAppStore = defineStore("app", {
  state: () => ({
    // App settings
    firstRun: true,
    skipWelcome: false,
    theme: "light",

    // System info
    os: "unknown",
    arch: "unknown",
    cpuCount: 0,
    additionalSystemInfo: {},
    eim_version: "unknown",

    // Installation status
    installedVersions: [],
    offlineArchives: [],
    prerequisitesInstalled: true,
    missingPrerequisites: [],

    // Current installation
    currentInstallation: {
      version: null,
      path: null,
      status: "store.installation.status.idle",
      progress: 0,
      message: "",
      tools: [],
    },

    // Configuration
    defaultConfig: {
      path: null,
      versions: [],
      tools: [],
      options: {},
    },
    // Prerequisites state
    prerequisitesChecking: false,
    prerequisitesLastChecked: null,
    prerequisitesStatus: {
      allOk: false,
      missing: [],
    },
  }),

  getters: {
    hasInstalledVersions: (state) => state.installedVersions.length > 0,
    hasOfflineArchives: (state) => state.offlineArchives.length > 0,
    isWindows: (state) => state.os === "windows",
    isMac: (state) => state.os === "macos",
    isLinux: (state) => state.os === "linux",
    installationInProgress: (state) =>
      [
        "store.installation.status.downloading",
        "store.installation.status.installing",
      ].includes(state.currentInstallation.status),
    canInstall: (state) =>
      state.prerequisitesInstalled || state.os === "windows",
  },

  actions: {
    async fetchSystemInfo() {
      const os = await invoke('get_operating_system')
      const arch = await invoke('get_system_arch')
      const cpuCount = await invoke('cpu_count')
      const additionalSystemInfo = await invoke('get_system_info')
      const app_info = await invoke('get_app_info')
      const eim_version = app_info.version
      const info = { os, arch, cpuCount , additionalSystemInfo , eim_version};
      this.setSystemInfo(info);
    },
    setSystemInfo(info) {
      this.os = info.os;
      this.arch = info.arch;
      this.cpuCount = info.cpuCount;
      this.additionalSystemInfo = info.additionalSystemInfo;
      this.eim_version = info.eim_version;
    },

    setInstalledVersions(versions) {
      this.installedVersions = versions;
    },

    addInstalledVersion(version) {
      this.installedVersions.push(version);
    },

    removeInstalledVersion(versionId) {
      this.installedVersions = this.installedVersions.filter(
        (v) => v.id !== versionId
      );
    },

    updateInstalledVersion(versionId, updates) {
      const index = this.installedVersions.findIndex((v) => v.id === versionId);
      if (index !== -1) {
        this.installedVersions[index] = {
          ...this.installedVersions[index],
          ...updates,
        };
      }
    },

    setOfflineArchives(archives) {
      this.offlineArchives = archives;
    },

    setPrerequisites(status, missing = []) {
      this.prerequisitesInstalled = status;
      this.missingPrerequisites = missing;
    },

    updateInstallationStatus(status) {
      this.currentInstallation = {
        ...this.currentInstallation,
        ...status,
      };
    },

    resetInstallation() {
      this.currentInstallation = {
        version: null,
        path: null,
        status: "store.installation.status.idle",
        progress: 0,
        message: "",
        tools: [],
      };
    },

    setAppSettings(settings) {
      if (settings.firstRun !== undefined) this.firstRun = settings.firstRun;
      if (settings.skipWelcome !== undefined)
        this.skipWelcome = settings.skipWelcome;
      if (settings.theme !== undefined) this.theme = settings.theme;
    },

    setDefaultConfig(config) {
      this.defaultConfig = config;
    },
    async getOs() {
      if (!this.os || this.os === 'unknown') {
        await this.fetchSystemInfo();
      }
      return this.os;
    },
    async getCpuCount() {
      if (!this.cpuCount || this.cpuCount === 0) {
        await this.fetchSystemInfo();
      }
      return this.cpuCount;
    },
    async getArch() {
      if (!this.arch || this.arch === 'unknown') {
        await this.fetchSystemInfo();
      }
      return this.arch;
    },
    async getEimVersion() {
      if (!this.eim_version || this.eim_version === 'unknown') {
        await this.fetchSystemInfo();
      }
      return this.eim_version;
    },
    async checkPrerequisites(force = false) {

      // Skip if already checking or recently checked (unless forced)
      if (this.prerequisitesChecking) {
        return this.prerequisitesStatus;
      }

      // if (!force && this.prerequisitesLastChecked) {
      //   const timeSinceLastCheck = Date.now() - this.prerequisitesLastChecked;
      //   // Skip if checked within last 1 minute
      //   if (timeSinceLastCheck < 1 * 60 * 1000) {
      //     return this.prerequisitesStatus;
      //   }
      // }

      this.prerequisitesChecking = true;

      try {
        const result = await invoke('check_prerequisites_detailed', {});

        this.prerequisitesStatus = {
          allOk: result.all_ok,
          missing: result.missing || [],
        };

        this.prerequisitesLastChecked = Date.now();

        // // Update the old format for backward compatibility
        // this.prerequisitesInstalled = result.all_ok;
        // this.missingPrerequisites = result.missing || [];

        return this.prerequisitesStatus;
      } catch (error) {
        console.error("Error checking prerequisites:", error);
        this.prerequisitesStatus = {
          allOk: false,
          missing: [],
        };
        return this.prerequisitesStatus;
      } finally {
        this.prerequisitesChecking = false;
      }
    },
    // Non-blocking background check
    checkPrerequisitesBackground() {
      // Fire and forget - don't await
      this.checkPrerequisites().catch(err => {
        console.error("Background prerequisite check failed:", err);
      });
    },
  },

  persist: {
    enabled: true,
    strategies: [
      {
        key: "esp-idf-installer-settings",
        storage: localStorage,
        paths: ["firstRun", "skipWelcome", "theme"],
      },
    ],
  },
});

export const useWizardStore = defineStore("wizard", {
  state: () => ({
    currentStep: 1,
    totalSteps: 9,
    wizardData: {
      // Step 1: Prerequisites
      prerequisites: {
        checked: false,
        allInstalled: false,
        missing: [],
      },

      // Step 2: Installation Path
      installPath: "",
      useDefaultPath: true,

      // Step 3: Version Selection
      selectedVersions: [],
      availableVersions: [],

      // Step 4: Tools Selection
      selectedTools: [],
      availableTools: [],

      // Step 5: Python Configuration
      pythonPath: "",
      pythonVersion: "",
      useBundledPython: true,

      // Step 6: Mirror Selection
      mirrorUrl: "",
      useDefaultMirror: true,

      // Step 7: Additional Options
      options: {
        createShortcuts: true,
        addToPath: true,
        installExamples: true,
        enableTelemetry: false,
      },
    },
  }),

  getters: {
    isFirstStep: (state) => state.currentStep === 1,
    isLastStep: (state) => state.currentStep === state.totalSteps,
    progressPercentage: (state) => (state.currentStep / state.totalSteps) * 100,
    canProceed: (state) => {
      // Add validation logic for each step
      switch (state.currentStep) {
        case 1:
          return (
            state.wizardData.prerequisites.allInstalled ||
            state.wizardData.prerequisites.checked
          );
        case 2:
          return state.wizardData.installPath !== "";
        case 3:
          return state.wizardData.selectedVersions.length > 0;
        case 4:
          return true; // Tools are optional
        case 5:
          return (
            state.wizardData.useBundledPython ||
            state.wizardData.pythonPath !== ""
          );
        case 6:
          return true; // Mirror is optional
        case 7:
          return true; // Options are optional
        case 8:
          return state.wizardData.configSummary !== null;
        default:
          return true;
      }
    },
  },

  actions: {
    nextStep() {
      if (this.currentStep < this.totalSteps) {
        this.currentStep++;
      }
    },

    previousStep() {
      if (this.currentStep > 1) {
        this.currentStep--;
      }
    },

    goToStep(step) {
      if (step >= 1 && step <= this.totalSteps) {
        this.currentStep = step;
      }
    },

    updateStepData(step, data) {
      switch (step) {
        case 1:
          this.wizardData.prerequisites = {
            ...this.wizardData.prerequisites,
            ...data,
          };
          break;
        case 2:
          if (data.installPath !== undefined)
            this.wizardData.installPath = data.installPath;
          if (data.useDefaultPath !== undefined)
            this.wizardData.useDefaultPath = data.useDefaultPath;
          break;
        case 3:
          if (data.selectedVersions !== undefined)
            this.wizardData.selectedVersions = data.selectedVersions;
          if (data.availableVersions !== undefined)
            this.wizardData.availableVersions = data.availableVersions;
          break;
        case 4:
          if (data.selectedTools !== undefined)
            this.wizardData.selectedTools = data.selectedTools;
          if (data.availableTools !== undefined)
            this.wizardData.availableTools = data.availableTools;
          break;
        case 5:
          if (data.pythonPath !== undefined)
            this.wizardData.pythonPath = data.pythonPath;
          if (data.pythonVersion !== undefined)
            this.wizardData.pythonVersion = data.pythonVersion;
          if (data.useBundledPython !== undefined)
            this.wizardData.useBundledPython = data.useBundledPython;
          break;
        case 6:
          if (data.mirrorUrl !== undefined)
            this.wizardData.mirrorUrl = data.mirrorUrl;
          if (data.useDefaultMirror !== undefined)
            this.wizardData.useDefaultMirror = data.useDefaultMirror;
          break;
        case 7:
          this.wizardData.options = {
            ...this.wizardData.options,
            ...data,
          };
          break;
        case 8:
          this.wizardData.configSummary = data;
          break;
      }
    },

    resetWizard() {
      this.currentStep = 1;
      this.wizardData = {
        prerequisites: {
          checked: false,
          allInstalled: false,
          missing: [],
        },
        installPath: "",
        useDefaultPath: true,
        selectedVersions: [],
        availableVersions: [],
        selectedTools: [],
        availableTools: [],
        pythonPath: "",
        pythonVersion: "",
        useBundledPython: true,
        mirrorUrl: "",
        useDefaultMirror: true,
        options: {
          createShortcuts: true,
          addToPath: true,
          installExamples: true,
          enableTelemetry: false,
        },
        configSummary: null,
      };
    },

    generateConfig() {
      return {
        path: this.wizardData.installPath,
        versions: this.wizardData.selectedVersions,
        tools: this.wizardData.selectedTools,
        python: {
          path: this.wizardData.pythonPath,
          version: this.wizardData.pythonVersion,
          useBundled: this.wizardData.useBundledPython,
        },
        mirror: {
          url: this.wizardData.mirrorUrl,
          useDefault: this.wizardData.useDefaultMirror,
        },
        options: this.wizardData.options,
      };
    },
  },
});

export const useMirrorsStore = defineStore("mirrors", {
  state: () => ({
    // URL lists
    idf_urls: [],
    tools_urls: [],
    pypi_urls: [],

    // Latency maps (url -> ms; 0 means timeout/unreachable; undefined means not yet measured)
    idf_latency_map: {},
    tools_latency_map: {},
    pypi_latency_map: {},

    // Selected (from backend quick URL endpoints)
    selected_idf: "",
    selected_tools: "",
    selected_pypi: "",

    // Loading flags
    loading_idf_urls: false,
    loading_tools_urls: false,
    loading_pypi_urls: false,
    loading_idf_latency: false,
    loading_tools_latency: false,
    loading_pypi_latency: false,

    // Last updated timestamps (ms epoch)
    idf_last_updated: 0,
    tools_last_updated: 0,
    pypi_last_updated: 0,

    // TTL for latency cache (15 minutes)
    latency_ttl_ms: 15 * 60 * 1000,
  }),
  getters: {
    idfUrls: (state) => state.idf_urls,
    toolsUrls: (state) => state.tools_urls,
    pypiUrls: (state) => state.pypi_urls,
    idfLatencyMap: (state) => state.idf_latency_map,
    toolsLatencyMap: (state) => state.tools_latency_map,
    pypiLatencyMap: (state) => state.pypi_latency_map,
  },
  actions: {
    // Backend uses Option<u32> for latency values; Timedout values are represented as None. 
    // We normalize to 0 for timeout and the value for the latency. If the value is undefined, we return undefined as it means the mirror is not yet measured.
    normalizeLatencyValue(value) {
      if (value === undefined) return undefined;
      if (value == null) return 0;
      return Number(value);
    },

    ttlValid(lastUpdated) {
      if (!lastUpdated) return false;
      const now = Date.now();
      return now - lastUpdated < this.latency_ttl_ms;
    },

    async bootstrapMirrors() {
      // Fetch quick URL lists + defaults for all types in parallel
      this.loading_idf_urls = true;
      this.loading_tools_urls = true;
      this.loading_pypi_urls = true;
      try {
        const pIdf = invoke("get_idf_mirror_urls", {});
        const pTools = invoke("get_tools_mirror_urls", {});
        const pPypi = invoke("get_pypi_mirror_urls", {});

        const [idf, tools, pypi] = await Promise.allSettled([pIdf, pTools, pPypi]);

        if (idf.status === "fulfilled") {
          const res = idf.value || {};
          this.idf_urls = Array.isArray(res.mirrors) ? res.mirrors : [];
          this.selected_idf = typeof res.selected === "string" ? res.selected : "";
        }
        if (tools.status === "fulfilled") {
          const res = tools.value || {};
          this.tools_urls = Array.isArray(res.mirrors) ? res.mirrors : [];
          this.selected_tools = typeof res.selected === "string" ? res.selected : "";
        }
        if (pypi.status === "fulfilled") {
          const res = pypi.value || {};
          this.pypi_urls = Array.isArray(res.mirrors) ? res.mirrors : [];
          this.selected_pypi = typeof res.selected === "string" ? res.selected : "";
        }
      } finally {
        this.loading_idf_urls = false;
        this.loading_tools_urls = false;
        this.loading_pypi_urls = false;
      }

      // Kick off progressive per-type background latency calculations
      this.computeLatencyInBackground();
    },

    computeLatencyInBackground() {
      const now = Date.now();
      // IDF
      if (!this.ttlValid(this.idf_last_updated) && !this.loading_idf_latency) {
        this.loading_idf_latency = true;
        invoke("get_idf_mirror_list", {})
          .then((res) => {
            const map = (res && res.mirrors) || {};
            const normalizedMap = {};
            Object.keys(map || {}).forEach((url) => {
              normalizedMap[url] = this.normalizeLatencyValue(map[url]);
            });
            this.idf_latency_map = normalizedMap;
            this.idf_last_updated = now;
          })
          .finally(() => {
            this.loading_idf_latency = false;
          });
      }

      // Tools
      if (!this.ttlValid(this.tools_last_updated) && !this.loading_tools_latency) {
        this.loading_tools_latency = true;
        invoke("get_tools_mirror_list", {})
          .then((res) => {
            const map = (res && res.mirrors) || {};
            const normalizedMap = {};
            Object.keys(map || {}).forEach((url) => {
              normalizedMap[url] = this.normalizeLatencyValue(map[url]);
            });
            this.tools_latency_map = normalizedMap;
            this.tools_last_updated = now;
          })
          .finally(() => {
            this.loading_tools_latency = false;
          });
      }

      // PyPI
      if (!this.ttlValid(this.pypi_last_updated) && !this.loading_pypi_latency) {
        this.loading_pypi_latency = true;
        invoke("get_pypi_mirror_list", {})
          .then((res) => {
            const map = (res && res.mirrors) || {};
            const normalizedMap = {};
            Object.keys(map || {}).forEach((url) => {
              normalizedMap[url] = this.normalizeLatencyValue(map[url]);
            });
            this.pypi_latency_map = normalizedMap;
            this.pypi_last_updated = now;
          })
          .finally(() => {
            this.loading_pypi_latency = false;
          });
      }
    },
  },
});
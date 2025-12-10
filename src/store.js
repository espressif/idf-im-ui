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
    mirrors: {
      idf: {
        urlsCmd: "get_idf_mirror_urls",
        latencyCmd: "get_idf_mirror_latency_entries",
        urls: [],
        entries: [],
        selected: "",
        loading_urls: false,
        loading_latency: false,
        last_updated: 0,
      },
      tools: {
        urlsCmd: "get_tools_mirror_urls",
        latencyCmd: "get_tools_mirror_latency_entries",
        urls: [],
        entries: [],
        selected: "",
        loading_urls: false,
        loading_latency: false,
        last_updated: 0,
      },
      pypi: {
        urlsCmd: "get_pypi_mirror_urls",
        latencyCmd: "get_pypi_mirror_latency_entries",
        urls: [],
        entries: [],
        selected: "",
        loading_urls: false,
        loading_latency: false,
        last_updated: 0,
      },
    },

    // TTL for latency cache (15 minutes)
    latency_ttl_ms: 15 * 60 * 1000,

  }),
  getters: {
    idfUrls: (state) => state.mirrors.idf.urls,
    toolsUrls: (state) => state.mirrors.tools.urls,
    pypiUrls: (state) => state.mirrors.pypi.urls,
    idfEntries: (state) => state.mirrors.idf.entries,
    toolsEntries: (state) => state.mirrors.tools.entries,
    pypiEntries: (state) => state.mirrors.pypi.entries,
  },
  actions: {
    getMirror(kind) {
      const mirror = this.mirrors[kind];
      if (!mirror) {
        console.error(`Unknown mirror type: ${kind}`);
      }
      return mirror;
    },
    async ttlValid(lastUpdated) {
      if (!lastUpdated) return false;
      const now = Date.now();
      return now - lastUpdated < this.latency_ttl_ms;
    },


    bootstrapMirrorsBackground() {
      this.bootstrapMirrors().catch(err => {
        console.error("Background mirror bootstrap failed:", err);
      });
    },

    async bootstrapMirrors() {
      // Fetch quick URL lists + defaults for all types in parallel
      console.log("Bootstrapping mirrors background...");
      await this.updateMirrors("idf");
      await this.updateMirrors("tools");
      await this.updateMirrors("pypi");
    },

    async updateMirrors(kind) {
      const mirror = this.getMirror(kind);
      if (!mirror || !mirror.urlsCmd) return;

      mirror.loading_urls = true;
      try {
        const res = await invoke(mirror.urlsCmd, {});
        mirror.urls = Array.isArray(res.mirrors) ? res.mirrors : [];
        mirror.selected = typeof res.selected === "string" ? res.selected : "";
        return this.updateMirrorLatency(kind);
      } catch (err) {
        console.error(`Failed to update ${kind} mirrors:`, err);
      } finally {
        mirror.loading_urls = false;
      }
    },

    async updateMirrorLatency(kind) {
      const mirror = this.getMirror(kind);
      if (!mirror || !mirror.latencyCmd) return;

      if (await this.ttlValid(mirror.last_updated) || mirror.loading_latency) {
        return;
      }

      mirror.loading_latency = true;
      try {
        const res = await invoke(mirror.latencyCmd, {});
        const entries = (res && res.entries) || [];
        mirror.entries = Array.isArray(entries) ? entries : [];
        mirror.last_updated = Date.now();
      } catch (err) {
        console.error(`Failed to compute ${kind} mirror latency:`, err);
      } finally {
        mirror.loading_latency = false;
      }
    },
  },
});
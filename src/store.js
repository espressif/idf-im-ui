import { defineStore } from "pinia";

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

    // Installation status
    installedVersions: [],
    offlineArchives: [],
    prerequisitesInstalled: true,
    missingPrerequisites: [],

    // Current installation
    currentInstallation: {
      version: null,
      path: null,
      status: "idle", // idle, downloading, installing, complete, error
      progress: 0,
      message: "",
      tools: []
    },

    // Configuration
    defaultConfig: {
      path: null,
      versions: [],
      tools: [],
      options: {}
    }
  }),

  getters: {
    hasInstalledVersions: (state) => state.installedVersions.length > 0,
    hasOfflineArchives: (state) => state.offlineArchives.length > 0,
    isWindows: (state) => state.os === "windows",
    isMac: (state) => state.os === "macos",
    isLinux: (state) => state.os === "linux",
    installationInProgress: (state) =>
      ["downloading", "installing"].includes(state.currentInstallation.status),
    canInstall: (state) =>
      state.prerequisitesInstalled || state.os === "windows"
  },

  actions: {
    setSystemInfo(info) {
      this.os = info.os;
      this.arch = info.arch;
      this.cpuCount = info.cpuCount;
    },

    setInstalledVersions(versions) {
      this.installedVersions = versions;
    },

    addInstalledVersion(version) {
      this.installedVersions.push(version);
    },

    removeInstalledVersion(versionId) {
      this.installedVersions = this.installedVersions.filter(v => v.id !== versionId);
    },

    updateInstalledVersion(versionId, updates) {
      const index = this.installedVersions.findIndex(v => v.id === versionId);
      if (index !== -1) {
        this.installedVersions[index] = {
          ...this.installedVersions[index],
          ...updates
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
        ...status
      };
    },

    resetInstallation() {
      this.currentInstallation = {
        version: null,
        path: null,
        status: "idle",
        progress: 0,
        message: "",
        tools: []
      };
    },

    setAppSettings(settings) {
      if (settings.firstRun !== undefined) this.firstRun = settings.firstRun;
      if (settings.skipWelcome !== undefined) this.skipWelcome = settings.skipWelcome;
      if (settings.theme !== undefined) this.theme = settings.theme;
    },

    setDefaultConfig(config) {
      this.defaultConfig = config;
    }
  },

  persist: {
    enabled: true,
    strategies: [
      {
        key: "esp-idf-installer-settings",
        storage: localStorage,
        paths: ["firstRun", "skipWelcome", "theme"]
      }
    ]
  }
});

export const useWizardStore = defineStore("wizard", {
  state: () => ({
    currentStep: 1,
    totalSteps: 8,
    wizardData: {
      // Step 1: Prerequisites
      prerequisites: {
        checked: false,
        allInstalled: false,
        missing: []
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
        enableTelemetry: false
      },

      // Step 8: Review & Confirm
      configSummary: null
    }
  }),

  getters: {
    isFirstStep: (state) => state.currentStep === 1,
    isLastStep: (state) => state.currentStep === state.totalSteps,
    progressPercentage: (state) => (state.currentStep / state.totalSteps) * 100,
    canProceed: (state) => {
      // Add validation logic for each step
      switch (state.currentStep) {
        case 1:
          return state.wizardData.prerequisites.allInstalled ||
                 state.wizardData.prerequisites.checked;
        case 2:
          return state.wizardData.installPath !== "";
        case 3:
          return state.wizardData.selectedVersions.length > 0;
        case 4:
          return true; // Tools are optional
        case 5:
          return state.wizardData.useBundledPython ||
                 state.wizardData.pythonPath !== "";
        case 6:
          return true; // Mirror is optional
        case 7:
          return true; // Options are optional
        case 8:
          return state.wizardData.configSummary !== null;
        default:
          return true;
      }
    }
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
            ...data
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
            ...data
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
          missing: []
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
          enableTelemetry: false
        },
        configSummary: null
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
          useBundled: this.wizardData.useBundledPython
        },
        mirror: {
          url: this.wizardData.mirrorUrl,
          useDefault: this.wizardData.useDefaultMirror
        },
        options: this.wizardData.options
      };
    }
  }
});

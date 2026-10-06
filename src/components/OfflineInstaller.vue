<template>
  <div class="offline-installer">
    <div class="installer-header">
      <h1 class="title">{{ $t('offlineInstaller.title') }}</h1>
      <n-button @click="goBack" type="primary" quaternary>
        <template #icon>
          <n-icon><ArrowLeftOutlined /></n-icon>
        </template>
        {{ $t('offlineInstaller.back') }}
      </n-button>
    </div>

    <!-- Archive Selection -->
    <n-card v-if="!installationStarted" class="config-card">
      <h2>{{ $t('offlineInstaller.config.title') }}</h2>
      <n-alert
        v-if="checkingPrerequisites"
        type="info"
        show-icon
        style="margin-bottom: 1rem;"
      >
        <template #icon>
          <n-spin size="small" />
        </template>
        {{ $t('offlineInstaller.config.prerequisites.checking') }}
      </n-alert>

      <n-alert
        v-if="missing_prerequisities.length > 0"
        type="error"
        show-icon
        style="margin-bottom: 1rem;"
      >
        <div>
          <div style="margin-bottom: 0.5rem;">
            {{ $t('offlineInstaller.config.prerequisites.missing') }} {{ missing_prerequisities.join(', ') }}
          </div>
          <n-alert
            v-if="operating_system === 'macos'"
            type="info"
            style="margin-top: 0.75rem;margin-bottom: 0.75rem;"
          >
            <strong>{{ $t('offlineInstaller.config.prerequisites.installCommand') }}:</strong>
            <pre style="margin-top: 0.5rem; padding: 0.5rem; background: #f5f5f5; border-radius: 4px; overflow-x: auto;">brew install {{ missing_prerequisities.join(' ') }}</pre>
          </n-alert>
          <n-alert
            v-if="operating_system === 'linux'"
            type="info"
            style="margin-top: 0.75rem;margin-bottom: 0.75rem;"
          >
            <strong>{{ $t('offlineInstaller.config.prerequisites.installCommand') }}:</strong>
            <pre style="margin-top: 0.5rem; padding: 0.5rem; background: #f5f5f5; border-radius: 4px; overflow-x: auto;">{{ $t('offlineInstaller.config.prerequisites.installCommand') }}: {{ missing_prerequisities.join(' ') }}</pre>
          </n-alert>
        </div>
      </n-alert>

      <!-- Selected Archives -->
      <div class="section" data-id="archive-section">
        <h3>{{ $t('offlineInstaller.config.archive.title') }}</h3>
        <div v-if="archives.length > 0" class="archive-list">
          <n-card v-for="(archive, index) in archives" :key="index" size="small">
            <div class="archive-item">
              <div class="archive-info">
                <n-icon size="24"><FileZipOutlined /></n-icon>
                <div>
                  <div class="archive-name">{{ getFileName(archive) }}</div>
                </div>
              </div>
              <n-button
                @click="removeArchive(index)"
                quaternary
                circle
                type="primary"
              >
                <template #icon>
                  <n-icon><CloseOutlined /></n-icon>
                </template>
              </n-button>
            </div>
          </n-card>
        </div>

        <n-button
          @click="addMoreArchives"
          dashed
          block
          style="margin-top: 1rem;"
          v-if="archives.length < 1"
        >
          <template #icon>
            <n-icon><PlusOutlined /></n-icon>
          </template>
          {{ $t('offlineInstaller.config.archive.addButton') }}
        </n-button>
      </div>

      <!-- Installation Path -->
      <div class="section" data-id="installation-path-section">
        <h3 data-id="installation-path-title">{{ $t('offlineInstaller.config.path.title') }}</h3>
        <n-input-group data-id="installation-path-input-group">
          <n-input
            v-model:value="installPath"
            :placeholder="$t('offlineInstaller.config.path.placeholder')"
            :disabled="useDefaultPath"
            data-id="installation-path-input"
          />
          <n-button @click="browsePath" :disabled="useDefaultPath" data-id="installation-path-browse-button">
            <template #icon>
              <n-icon><FolderOpenOutlined /></n-icon>
            </template>
            {{ $t('offlineInstaller.config.archive.browse') }}
          </n-button>
        </n-input-group>
        <n-checkbox
          v-model:checked="useDefaultPath"
          style="margin-top: 0.5rem;"
          data-id="installation-path-use-default-checkbox"
        >
          {{ $t('offlineInstaller.config.path.useDefault') }}
        </n-checkbox>
        <n-alert
          v-if="!pathValid && installPath"
          type="warning"
          style="margin-top: 1rem;"
          data-id="installation-path-warning"
        >
          {{ $t('offlineInstaller.config.path.warning') }}
        </n-alert>

        <div class="advanced-section" data-id="advanced-options-section">
          <div class="option-row" data-id="cleanup-option-row">
            <n-checkbox
              v-model:checked="cleanup"
              @update:checked="onCleanupChange"
              data-id="cleanup-checkbox"
            >
              {{ $t('installationPathSelect.cleanup.label') }}
            </n-checkbox>
            <p class="option-warning" v-if="cleanup" data-id="cleanup-warning">
              {{ $t('installationPathSelect.cleanup.warning') }}
            </p>
          </div>

          <div class="option-row" data-id="custom-folders-option-row">
            <n-checkbox
              v-model:checked="customToolFolders"
              @update:checked="onCustomFoldersToggle"
              data-id="custom-folders-checkbox"
            >
              {{ $t('installationPathSelect.toolFolders.enableLabel') }}
            </n-checkbox>
            <p class="option-warning" v-if="customToolFolders" data-id="custom-folders-warning">
              {{ $t('installationPathSelect.toolFolders.warning') }}
            </p>
          </div>

          <div class="folder-inputs" data-id="custom-folders-inputs">
            <div class="folder-field" data-id="tool-download-folder-field">
              <label class="folder-label">
                {{ $t('installationPathSelect.toolFolders.downloadLabel') }}
              </label>
              <n-input-group>
                <n-input
                  v-model:value="toolDownloadFolderName"
                  :placeholder="$t('installationPathSelect.toolFolders.downloadPlaceholder')"
                  :disabled="!customToolFolders"
                  data-id="tool-download-folder-input"
                />
                <n-button
                  @click="browseToolFolder('download')"
                  type="error"
                  :disabled="!customToolFolders"
                  data-id="tool-download-browse-button"
                >
                  {{ $t('offlineInstaller.config.archive.browse') }}
                </n-button>
              </n-input-group>
            </div>
            <div class="folder-field" data-id="tool-install-folder-field">
              <label class="folder-label">
                {{ $t('installationPathSelect.toolFolders.installLabel') }}
              </label>
              <n-input-group>
                <n-input
                  v-model:value="toolInstallFolderName"
                  :placeholder="$t('installationPathSelect.toolFolders.installPlaceholder')"
                  :disabled="!customToolFolders"
                  data-id="tool-install-folder-input"
                />
                <n-button
                  @click="browseToolFolder('install')"
                  type="error"
                  :disabled="!customToolFolders"
                  data-id="tool-install-browse-button"
                >
                  {{ $t('offlineInstaller.config.archive.browse') }}
                </n-button>
              </n-input-group>
            </div>
          </div>
        </div>
      </div>

      <!-- Action Buttons -->
      <div class="actions">
        <n-button @click="goBack" size="large">
          {{ $t('offlineInstaller.cancel') }}
        </n-button>
        <n-button
          @click="startInstallation"
          type="primary"
          size="large"
          data-id="start-installation-button"
          :disabled="archives.length === 0 || !installPath || missing_prerequisities.length > 0"
        >
          {{ $t('offlineInstaller.config.startButton') }}
        </n-button>
      </div>
    </n-card>

    <!-- Installation Progress -->
    <n-card v-else class="progress-card" data-id="offline-installation-progress">
      <h2 data-id="offline-installation-title">{{ $t('offlineInstaller.installation.title') }}</h2>

      <n-alert title="Installation Error" type="error" v-if="error_message" data-id="offline-installation-error-alert">
        {{ error_message }}
      </n-alert>

      <!-- Current Activity Display -->
      <installation-activity
        v-if="installation_running"
        :title="$t('offlineInstaller.installation.currentActivity')"
        :activity="currentActivity"
        :detail="currentDetail"
        :progress-label="$t('offlineInstaller.installation.overallProgress')"
        :progress="currentProgress"
        :processing="installation_running"
        :steps="installationSteps"
        :current-step="currentStep"
      />

      <!-- Error State -->
      <div v-if="installation_failed" class="error-message" data-id="error-message">
        <h3 data-id="error-title">{{ $t('offlineInstaller.installation.error.title') }}</h3>
        <p data-id="error-message-text">{{ error_message }} <br> {{ $t('offlineInstaller.installation.error.info') }}</p>
        <n-button @click="retry" type="primary" size="large" data-id="retry-installation-button">
          {{ $t('offlineInstaller.installation.error.retry') }}
        </n-button>
        <n-button @click="goBack" type="default" size="large" style="margin-left: 1rem;" data-id="back-installation-button">
          {{ $t('offlineInstaller.installation.error.back') }}
        </n-button>
      </div>

      <!-- Completion Actions -->
      <div class="action-footer" v-if="installation_finished && !installation_failed" data-id="action-footer">
        <n-button @click="finish" type="primary" size="large" data-id="complete-installation-button-footer">
          {{ $t('offlineInstaller.installation.success.complete') }}
        </n-button>
      </div>

      <!-- Installation Summary -->
      <div v-if="installation_finished && !installation_failed" class="installation-summary" data-id="installation-summary">
        <h3>{{ $t('offlineInstaller.installation.summary.title') }}</h3>
        <p>{{ $t('offlineInstaller.installation.summary.success') }}</p>
        <div class="summary-details">
          <div v-if="installed_versions.length > 0">
            <strong>{{ $t('offlineInstaller.installation.summary.installedVersions') }}:</strong> {{ installed_versions.join(', ') }}
          </div>
          <div v-if="installationPath">
            <strong>{{ $t('offlineInstaller.installation.summary.path') }}:</strong> {{ installationPath }}
          </div>
        </div>
      </div>

      <!-- Installation Log with Virtual Scrolling -->
      <installation-log
        v-if="totalLogCount > 0"
        ref="virtualContainer"
        :title="$t('offlineInstaller.installation.log.title')"
        :count-label="`(${totalLogCount} entries)`"
        v-bind="logListProps"
        @scroll="onLogScroll"
      />
    </n-card>
  </div>
</template>

<script>
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event'
import {
  NButton, NCard, NIcon, NInput, NInputGroup, NCheckbox,
  NSpace, NAlert, NSpin, NProgress, NSteps, NStep,
  NCollapse, NCollapseItem, NScrollbar, useMessage
} from 'naive-ui'
import {
  ArrowLeftOutlined, FolderOpenOutlined, PlusOutlined,
  CloseOutlined, FileZipOutlined, CheckCircleOutlined,
  CloseCircleOutlined
} from '@vicons/antd'
import { useI18n } from 'vue-i18n'
import { useAppStore } from '../store'
import {
  createAdvancedPathOptionsState,
  loadAdvancedPathOptions,
  onCleanupChange,
  onCustomFoldersToggle,
  browseToolFolder,
  persistAdvancedPathOptions,
} from './composables/useAdvancedPathOptions.js'
import { installationProgressMixin } from './composables/installationProgressMixin.js'
import InstallationActivity from './InstallationActivity.vue'
import InstallationLog from './InstallationLog.vue'

export default {
  name: 'OfflineInstaller',
  mixins: [installationProgressMixin],
  components: {
    NButton, NCard, NIcon, NInput, NInputGroup, NCheckbox,
    NSpace, NAlert, NSpin, NProgress, NSteps, NStep,
    NCollapse, NCollapseItem, NScrollbar,
    ArrowLeftOutlined, FolderOpenOutlined, PlusOutlined,
    CloseOutlined, FileZipOutlined, CheckCircleOutlined,
    CloseCircleOutlined,
    InstallationActivity, InstallationLog
  },

  data() {
    return {
      // Configuration
      archives: [],
      installPath: '',
      useDefaultPath: true,
      pathValid: true,
      operating_system: '',

      // Advanced tool-folder + cleanup options; see composable for
      // the canonical state shape and load/persist helpers. Same
      // surface as InstallationPathSelect.vue (EIM-863).
      ...createAdvancedPathOptionsState(),

      // Installation state
      installationStarted: false,
      installation_running: false,
      installation_finished: false,
      installation_failed: false,
      error_message: '',

      // Progress tracking
      currentStep: 0,
      currentStage: 'checking',
      installed_versions: [],

      // Installation steps
      installationSteps: [
        { title: this.$t('offlineInstaller.installation.steps.check.title'), description: this.$t('offlineInstaller.installation.steps.check.description') },
        { title: this.$t('offlineInstaller.installation.steps.extract.title'), description: this.$t('offlineInstaller.installation.steps.extract.description') },
        { title: this.$t('offlineInstaller.installation.steps.prerequisites.title'), description: this.$t('offlineInstaller.installation.steps.prerequisites.description') },
        { title: this.$t('offlineInstaller.installation.steps.install.title'), description: this.$t('offlineInstaller.installation.steps.install.description') },
        { title: this.$t('offlineInstaller.installation.steps.tools.title'), description: this.$t('offlineInstaller.installation.steps.tools.description') },
        { title: this.$t('offlineInstaller.installation.steps.python.title'), description: this.$t('offlineInstaller.installation.steps.python.description') },
        { title: this.$t('offlineInstaller.installation.steps.configure.title'), description: this.$t('offlineInstaller.installation.steps.configure.description') },
        { title: this.$t('offlineInstaller.installation.steps.complete.title'), description: this.$t('offlineInstaller.installation.steps.complete.description') }
      ],

      // Event listeners
      unlistenProgress: null,
      unlistenLog: null,

      timeStarted: null,

      missing_prerequisities: [],
      checkingPrerequisites: false,

      appStore: useAppStore(),
    }
  },

  created() {
    this._progressData = {
      currentProgress: 0,
      currentActivity: "Preparing offline installation...",
      currentDetail: "",
      lastUpdate: Date.now()
    };
  },

  computed: {
    currentActivity() {
      this.progressUpdateTrigger;
      return this._progressData ? this._progressData.currentActivity : "Preparing offline installation...";
    }
  },

  methods: {
    loadArchivesFromQuery() {
      if (this.$route.query.archives) {
        console.log('Loading archives from query:', this.$route.query.archives)
        try {
          this.archives = JSON.parse(this.$route.query.archives)
        } catch (e) {
          console.error('Failed to parse archives:', e)
        }
      }
    },

    async getDefaultPath() {
      try {
        const settings = await invoke('get_settings')
        this.installPath = settings?.path || ''
      } catch (error) {
        console.error('Failed to get default path:', error)
      }
    },

    getFileName(path) {
      return path.split(/[/\\]/).pop()
    },

    removeArchive(index) {
      this.archives.splice(index, 1)
    },

    async addMoreArchives() {
      try {
        const selected = await open({
          multiple: true,
          filters: [{
            name: 'Archive Files',
            extensions: ['zst']
          }]
        })

        if (selected) {
          const newArchives = Array.isArray(selected) ? selected : [selected]
          this.archives.push(...newArchives)
        }
      } catch (error) {
        this.$message.error(this.$t('offlineInstaller.messages.errors.selectArchives'))
      }
    },

    async browsePath() {
      try {
        const selected = await open({
          directory: true,
          multiple: false
        })

        if (selected) {
          this.installPath = selected
          await this.validatePath()
        }
      } catch (error) {
        this.$message.error(this.$t('offlineInstaller.messages.errors.selectPath'))
      }
    },

    async validatePath() {
      try {
        this.pathValid = await invoke('is_path_empty_or_nonexistent_command', {
          path: this.installPath
        })
      } catch (error) {
        this.pathValid = false
      }
    },

    onCleanupChange(checked) {
      return onCleanupChange(this, checked);
    },

    onCustomFoldersToggle(checked) {
      onCustomFoldersToggle(this, checked);
    },

    browseToolFolder(which) {
      return browseToolFolder(this, which);
    },

    async startListening() {
      // Listen for installation progress events
      this.unlistenProgress = await listen('installation-progress', (event) => {
        this.handleProgressEvent(event.payload);
      });

      // Listen for log messages
      this.unlistenLog = await listen('log-message', (event) => {
        console.log('Log message received:', event.payload);
        this.handleLogMessage(event.payload);
      });
    },

    handleProgressEvent(payload) {
      const { stage, percentage, message, detail, version } = payload;
      const now = Date.now();

      // Store in non-reactive object (no memory leak)
      this._progressData.currentProgress = percentage || 0;
      this._progressData.currentActivity = message || this._progressData.currentActivity;
      this._progressData.currentDetail = detail || "";
      this._progressData.lastUpdate = now;

      // Update stage (reactive, but changes rarely)
      if (stage !== this.currentStage) {
        this.currentStage = stage;
      }

      let newStep = this.currentStep;

      switch (stage) {
        case 'checking': newStep = 0; break;
        case 'extract': newStep = 1; break;
        case 'prerequisites': newStep = 2; break;
        case 'download': newStep = 3; break;
        case 'tools': newStep = 4; break;
        case 'python': newStep = 5; break;
        case 'configure': newStep = 6; break;
        case 'complete':
          newStep = 7;
          this.handleInstallationComplete(version);
          break;
        case 'error':
          this.handleInstallationError(message, detail);
          break;
      }

      // Only update reactive step if it actually changed
      if (newStep !== this.currentStep) {
        this.currentStep = newStep;
      }

      // Throttle UI updates
      this.throttledProgressUpdate();
    },

    handleInstallationComplete(version) {
      this.installation_running = false;
      this.installation_finished = true;

      if (version && !this.installed_versions.includes(version)) {
        this.installed_versions.push(version);
      }

      try {
        invoke("track_event_command", {
          event: "install_finished",
          mode: "offline",
          outcome: "success",
          versions: [version]
        });
      } catch (error) {
        console.warn('Failed to track event:', error);
      }
    },

    handleInstallationError(message, detail) {
      this.installation_running = false;
      this.installation_failed = true;
      this.error_message = message || "Offline installation failed";

      try {
        invoke("track_event_command", {
          event: "install_finished",
          mode: "offline",
          outcome: "failure",
          error_message: detail || message
        });
      } catch (error) {
        console.warn('Failed to track event:', error);
      }
    },

    async startInstallation() {
      this.installationStarted = true;
      this.installation_running = true;
      this.installation_finished = false;
      this.installation_failed = false;
      this.error_message = "";
      this.installed_versions = [];
      this.timeStarted = new Date();

      try {
        await invoke("track_event_command", {
          event: "install_started",
          mode: "offline"
        });
      } catch (error) {
        console.warn('Failed to track event:', error);
      }

      // Reset progress data
      this._progressData = {
        currentProgress: 0,
        currentActivity: "Starting offline installation...",
        currentDetail: "",
        lastUpdate: Date.now()
      };

      this.currentStep = 0;
      this.currentStage = 'checking';
      this.progressUpdateTrigger++;

      // Clear logs
      this.resetLogs();

      try {
        await persistAdvancedPathOptions(this);
      } catch (error) {
        console.warn('Failed to persist advanced path options:', error);
      }

      try {
        await invoke('start_offline_installation', {
          archives: this.archives,
          installPath: this.useDefaultPath ? "" : this.installPath,
        });
      } catch (error) {
        console.error('Offline installation failed:', error);
        this.error_message = error.toString();
        this.installation_failed = true;
        this.installation_running = false;
      }
    },

    retry() {
      this.installationStarted = false;
      this.installation_running = false;
      this.installation_finished = false;
      this.installation_failed = false;
      this.error_message = "";
      this.currentStep = 0;
      this.installed_versions = [];

      // Reset progress data
      this._progressData = {
        currentProgress: 0,
        currentActivity: "Preparing offline installation...",
        currentDetail: "",
        lastUpdate: Date.now()
      };
      this.progressUpdateTrigger++;
    },

    finish() {
      if (this.installation_finished) {
        this.$router.push('/version-management');
      } else {
        this.goBack();
      }
    },

    goBack() {
      this.$router.push('/basic-installer');
    },

    cleanupComponent() {
      if (this._progressThrottle) {
        clearTimeout(this._progressThrottle);
        this._progressThrottle = null;
      }
      this._progressData = null;

      if (this.unlistenProgress) {
        this.unlistenProgress();
        this.unlistenProgress = null;
      }
      if (this.unlistenLog) {
        this.unlistenLog();
        this.unlistenLog = null;
      }

      this._allLogs = null;
    },

    check_prerequisites: async function () {
      this.operating_system = await this.appStore.getOs();

      if (this.operating_system == 'windows') {
        this.missing_prerequisities = [];
        return false;
      }
      this.checkingPrerequisites = true;
      console.log("Checking prerequisites via store...");

      if (!this.appStore.prerequisitesChecking && this.appStore.prerequisitesLastChecked !== null) {
        this.missing_prerequisities = this.appStore.prerequisitesStatus.missing || [];
        console.log("Prerequisites already checked, using cached result:", this.missing_prerequisities);
        this.checkingPrerequisites = false;
      } else {
        console.log("Prerequisites not checked yet, invoking store check...");
        this.appStore.checkPrerequisites().then(() => {
          this.missing_prerequisities = this.appStore.prerequisitesStatus.missing || [];
          console.log("Prerequisites check completed via store:", this.missing_prerequisities);
        }).catch(error => {
          console.error("Error checking prerequisites via store:", error);
        });
        this.checkingPrerequisites = false;
        return false;
      }
    },
  },

  async mounted() {
    this.loadArchivesFromQuery();
    if (this.useDefaultPath) {
      await this.getDefaultPath();
    }
    await loadAdvancedPathOptions(this);
    await this.startListening();
    this.measureContainer();
    window.addEventListener('resize', this.measureContainer);
    this.check_prerequisites();
  },

  beforeUnmount() {
    this.cleanupComponent();
    window.removeEventListener('resize', this.measureContainer);
  }
}
</script>

<style scoped src="./styles/page-header.css"></style>

<style scoped>
.offline-installer {
  padding: 2rem;
  max-width: 1000px;
  margin: 0 auto;
}

.config-card, .progress-card {
  background: white;
  padding: 2rem;
  display: flex;
  flex-direction: column;
  align-content: center;
}

.config-card h2, .progress-card h2 {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 1.5rem;
  color: #374151;
  margin: 0 0 2rem 0;
}

.section {
  margin-bottom: 2rem;
}

.section h3 {
  font-family: 'Trueno-regular', sans-serif;
  font-size: 1.125rem;
  color: #4b5563;
  margin-bottom: 1rem;
}

.advanced-section {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  border-top: 1px solid #e5e7eb;
  padding-top: 1.25rem;
  margin-top: 1.25rem;
}

.archive-list {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.archive-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.5rem;
}

.archive-info {
  display: flex;
  align-items: center;
  gap: 1rem;
}

.archive-name {
  font-weight: 500;
  color: #1f2937;
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 1rem;
  padding-top: 2rem;
  border-top: 1px solid #e5e7eb;
}

/* Button styling */
.n-button {
  color: #e5e7eb;
}
</style>

<style scoped src="./styles/advanced-path-options.css"></style>

<style scoped src="./styles/installation-result.css"></style>

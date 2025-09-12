<template>
  <div class="offline-installer">
    <div class="installer-header">
      <h1 class="title">Offline Installation</h1>
      <n-button @click="goBack" type="primary" quaternary>
        <template #icon>
          <n-icon><ArrowLeftOutlined /></n-icon>
        </template>
        Back
      </n-button>
    </div>

    <!-- Archive Selection -->
    <n-card v-if="!installationStarted" class="config-card">
      <h2>Installation Configuration</h2>

      <!-- Selected Archives -->
      <div class="section">
        <h3>Selected Archive</h3>
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
          Add Archive
        </n-button>
      </div>

      <!-- Installation Path -->
      <div class="section">
        <h3>Installation Path</h3>
        <n-input-group>
          <n-input
            v-model:value="installPath"
            placeholder="Select installation directory"
            :disabled="useDefaultPath"
          />
          <n-button @click="browsePath" :disabled="useDefaultPath">
            <template #icon>
              <n-icon><FolderOpenOutlined /></n-icon>
            </template>
            Browse
          </n-button>
        </n-input-group>
        <n-checkbox
          v-model:checked="useDefaultPath"
          style="margin-top: 0.5rem;"
        >
          Use default installation path
        </n-checkbox>
        <n-alert
          v-if="!pathValid && installPath"
          type="warning"
          style="margin-top: 1rem;"
        >
          The selected path is not empty. Installation may fail if files already exist.
        </n-alert>
      </div>

      <!-- Action Buttons -->
      <div class="actions">
        <n-button @click="goBack" size="large">
          Cancel
        </n-button>
        <n-button
          @click="startInstallation"
          type="primary"
          size="large"
          :disabled="archives.length === 0 || !installPath"
        >
          Start Installation
        </n-button>
      </div>
    </n-card>

    <!-- Installation Progress -->
    <n-card v-else class="progress-card">
      <h2>Installing ESP-IDF from Offline Archive</h2>

      <n-alert title="Installation Error" type="error" v-if="error_message">
        {{ error_message }}
      </n-alert>

      <!-- Current Activity Display -->
      <div v-if="installation_running" class="current-activity" data-id="current-activity">
        <div class="current-step">
          <h3>Current Activity:</h3>
          <div class="activity-status">{{ currentActivity }}</div>
          <div v-if="currentDetail" class="activity-detail">{{ currentDetail }}</div>
        </div>

        <div class="progress-section">
          <div class="progress-label">Overall Progress</div>
          <n-progress
            type="line"
            :percentage="currentProgress"
            :processing="installation_running"
            :indicator-placement="'inside'"
            color="#E8362D"
          />
        </div>

        <!-- Installation Steps -->
        <div class="installation-steps" v-if="installationSteps.length > 0">
          <div class="steps-container">
            <div
              v-for="(step, index) in installationSteps"
              :key="index"
              class="step-item"
              :class="{
                'active': index === currentStep,
                'completed': index < currentStep,
                'pending': index > currentStep
              }"
            >
              <div class="step-indicator">{{ index + 1 }}</div>
              <div class="step-content">
                <div class="step-title">{{ step.title }}</div>
                <div class="step-description">{{ step.description }}</div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Error State -->
      <div v-if="installation_failed" class="error-message" data-id="error-message">
        <h3 data-id="error-title">Error during offline installation:</h3>
        <p data-id="error-message-text">{{ error_message }} <br> For more information consult the log file.</p>
        <n-button @click="retry" type="error" size="large" data-id="retry-installation-button">Retry Installation</n-button>
        <n-button @click="goBack" type="default" size="large" style="margin-left: 1rem;" data-id="back-installation-button">Go Back</n-button>
      </div>

      <!-- Completion Actions -->
      <div class="action-footer" v-if="installation_finished && !installation_failed" data-id="action-footer">
        <n-button @click="finish" type="error" size="large" data-id="complete-installation-button-footer">
          Complete Installation
        </n-button>
      </div>

      <!-- Installation Summary -->
      <div v-if="installation_finished && !installation_failed" class="installation-summary" data-id="installation-summary">
        <h3>Offline Installation Complete</h3>
        <p>Successfully installed ESP-IDF and all required tools from offline archive.</p>
        <div class="summary-details">
          <div v-if="installed_versions.length > 0">
            <strong>Installed Versions:</strong> {{ installed_versions.join(', ') }}
          </div>
          <div v-if="installationPath">
            <strong>Installation Path:</strong> {{ installationPath }}
          </div>
        </div>
      </div>

      <!-- Installation Log with Virtual Scrolling -->
      <n-collapse arrow-placement="right" v-if="totalLogCount > 0">
        <n-collapse-item title="Installation Log" name="1">
          <template #header-extra>
            <span class="log-count">({{ totalLogCount }} entries)</span>
          </template>

          <div class="log-container">
            <div
              class="log-virtual-container"
              ref="virtualContainer"
              @scroll="onLogScroll"
            >
              <div
                class="virtual-spacer-top"
                :style="{ height: topSpacerHeight + 'px' }"
              ></div>

              <div class="log-scroll-container">
                <div
                  v-for="(message, index) in visibleLogs"
                  :key="`log-${startIndex + index}-${message.timestamp}`"
                  class="log-entry"
                  :style="{ height: itemHeight + 'px' }"
                >
                  <pre
                    class="log-message"
                    :class="getLogMessageClass(message)"
                    v-text="message.text"
                  ></pre>
                </div>
              </div>

              <div
                class="virtual-spacer-bottom"
                :style="{ height: bottomSpacerHeight + 'px' }"
              ></div>
            </div>
          </div>
        </n-collapse-item>
      </n-collapse>
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

export default {
  name: 'OfflineInstaller',
  components: {
    NButton, NCard, NIcon, NInput, NInputGroup, NCheckbox,
    NSpace, NAlert, NSpin, NProgress, NSteps, NStep,
    NCollapse, NCollapseItem, NScrollbar,
    ArrowLeftOutlined, FolderOpenOutlined, PlusOutlined,
    CloseOutlined, FileZipOutlined, CheckCircleOutlined,
    CloseCircleOutlined
  },

  data() {
    return {
      // Configuration
      archives: [],
      installPath: '',
      useDefaultPath: true,
      pathValid: true,

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
        { title: 'Check', description: 'Validating archives' },
        { title: 'Extract', description: 'Extracting archive contents' },
        { title: 'Prerequisites', description: 'Installing dependencies' },
        { title: 'Install', description: 'Installing ESP-IDF' },
        { title: 'Tools', description: 'Setting up development tools' },
        { title: 'Python', description: 'Configuring Python environment' },
        { title: 'Configure', description: 'Finalizing configuration' },
        { title: 'Complete', description: 'Installation complete' }
      ],

      // Event listeners
      unlistenProgress: null,
      unlistenLog: null,

      // Virtual scrolling for logs
      visibleLogs: [],
      totalLogCount: 0,
      scrollTop: 0,
      containerHeight: 300,
      itemHeight: 24,
      visibleCount: 15,
      startIndex: 0,
      BUFFER_SIZE: 2,

      // UI state
      installationPath: "",

      // Progress tracking
      progressUpdateTrigger: 0,
      lastProgressUpdate: 0,
      timeStarted: null
    }
  },

  created() {
    this._allLogs = [];
    this._progressData = {
      currentProgress: 0,
      currentActivity: "Preparing offline installation...",
      currentDetail: "",
      lastUpdate: Date.now()
    };
    this._progressThrottle = null;
  },

  computed: {
    // Calculate spacer heights for virtual scrolling
    topSpacerHeight() {
      return this.startIndex * this.itemHeight;
    },

    bottomSpacerHeight() {
      const remainingItems = Math.max(0, this.totalLogCount - (this.startIndex + this.visibleLogs.length));
      return remainingItems * this.itemHeight;
    },

    // Calculate how many items can fit in the container
    maxVisibleItems() {
      return Math.ceil(this.containerHeight / this.itemHeight) + (this.BUFFER_SIZE * 2);
    },

    currentProgress() {
      this.progressUpdateTrigger;
      return this._progressData ? this._progressData.currentProgress : 0;
    },

    currentActivity() {
      this.progressUpdateTrigger;
      return this._progressData ? this._progressData.currentActivity : "Preparing offline installation...";
    },

    currentDetail() {
      this.progressUpdateTrigger;
      return this._progressData ? this._progressData.currentDetail : "";
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
        this.$message.error('Failed to select archives')
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
        this.$message.error('Failed to select path')
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

    throttledProgressUpdate() {
      if (this._progressThrottle) {
        clearTimeout(this._progressThrottle);
      }

      this._progressThrottle = setTimeout(() => {
        const now = Date.now();
        if (now - this.lastProgressUpdate > 100) {
          this.progressUpdateTrigger++;
          this.lastProgressUpdate = now;
        }
        this._progressThrottle = null;
      }, 100);
    },

    handleLogMessage(payload) {
      const { level, message } = payload;

      const logEntry = {
        level,
        text: message,
        timestamp: Date.now(),
        id: this._allLogs.length
      };

      this._allLogs.unshift(logEntry);

      const MAX_LOG_ENTRIES = 1000;
      if (this._allLogs.length > MAX_LOG_ENTRIES) {
        this._allLogs = this._allLogs.slice(0, MAX_LOG_ENTRIES);
      }

      this.totalLogCount = this._allLogs.length;
      this.updateVisibleLogs();

      if (this.scrollTop < this.itemHeight) {
        this.scrollToTop();
      }

      // Extract installation path from logs if available
      if (message.includes('installed at:') || message.includes('Location:')) {
        const pathMatch = message.match(/(?:installed at:|Location:)\s*(.+)/i);
        if (pathMatch && pathMatch[1]) {
          this.installationPath = pathMatch[1].trim();
        }
      }
    },

    updateVisibleLogs() {
      const startIndex = Math.max(0, Math.floor(this.scrollTop / this.itemHeight) - this.BUFFER_SIZE);
      const endIndex = Math.min(
        startIndex + this.maxVisibleItems,
        this._allLogs.length
      );

      this.startIndex = startIndex;
      this.visibleLogs = this._allLogs.slice(startIndex, endIndex).map(log => ({
        ...log
      }));
    },

    onLogScroll(event) {
      const newScrollTop = event.target.scrollTop;

      if (Math.abs(newScrollTop - this.scrollTop) > this.itemHeight / 2) {
        this.scrollTop = newScrollTop;
        this.updateVisibleLogs();
      }
    },

    scrollToTop() {
      this.$nextTick(() => {
        const container = this.$refs.virtualContainer;
        if (container) {
          container.scrollTop = 0;
          this.scrollTop = 0;
          this.updateVisibleLogs();
        }
      });
    },

    getLogMessageClass(message) {
      if (message.level === 'error') return 'log-message log-error';
      if (message.level === 'warning') return 'log-message log-warning';
      if (message.level === 'success') return 'log-message log-success';
      if (message.text && (message.text.includes('WARN') || message.text.includes('ERR'))) {
        return 'log-message highlight';
      }
      return 'log-message';
    },

    handleInstallationComplete(version) {
      this.installation_running = false;
      this.installation_finished = true;

      if (version && !this.installed_versions.includes(version)) {
        this.installed_versions.push(version);
      }

      // Add this tracking block
      try {
        invoke("track_event_command", {
          name: "GUI offline installation succeeded",
          additional_data: {
            duration_seconds: (new Date() - this.timeStarted) / 1000,
            version: version
          }
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
          name: "GUI offline installation failed",
          additional_data: {
            duration_seconds: (new Date() - this.timeStarted) / 1000,
            errorMessage: message,
            errorDetails: detail
          }
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
      this.timeStarted = new Date(); // Add this line

      try { // tracking should never fail installation
        await invoke("track_event_command", { name: "GUI offline installation started" });
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
      this._allLogs = [];
      this.visibleLogs = [];
      this.totalLogCount = 0;
      this.scrollTop = 0;
      this.startIndex = 0;

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

    measureContainer() {
      this.$nextTick(() => {
        const container = this.$refs.virtualContainer;
        if (container) {
          this.containerHeight = container.clientHeight;
          this.updateVisibleLogs();
        }
      });
    },

    cleanup() {
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
    }
  },

  async mounted() {
    this.loadArchivesFromQuery();
    if (this.useDefaultPath) {
      await this.getDefaultPath();
    }
    await this.startListening();
    this.measureContainer();
    window.addEventListener('resize', this.measureContainer);
  },

  beforeUnmount() {
    this.cleanup();
    window.removeEventListener('resize', this.measureContainer);
  }
}
</script>

<style scoped>
.offline-installer {
  padding: 2rem;
  max-width: 1000px;
  margin: 0 auto;
}

.installer-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 2rem;
}

.title {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 2rem;
  color: #1f2937;
  margin: 0;
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

/* Current Activity Display */
.current-activity {
  margin: 1rem 0;
  padding: 1rem;
  background-color: #f9fafb;
  border-radius: 8px;
  border-left: 4px solid #428ED2;
}

.current-step h3 {
  margin: 0 0 0.5rem 0;
  font-size: 1rem;
  color: #6b7280;
}

.activity-status {
  font-size: 1.1rem;
  font-weight: 500;
  color: #374151;
}

.activity-detail {
  font-size: 0.9rem;
  color: #6b7280;
  margin-top: 0.5rem;
}

.progress-section {
  margin-top: 1rem;
}

.progress-label {
  font-size: 0.875rem;
  color: #6b7280;
  margin-bottom: 0.5rem;
}

/* Installation Steps */
.installation-steps {
  margin-top: 1.5rem;
}

.steps-container {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 1rem;
}

.step-item {
  display: flex;
  align-items: center;
  padding: 0.75rem;
  border-radius: 8px;
  border: 1px solid #e5e7eb;
  transition: all 0.2s ease;
}

.step-item.active {
  border-color: #428ED2;
  background-color: #eff6ff;
}

.step-item.completed {
  border-color: #10b981;
  background-color: #f0fdf4;
}

.step-item.completed .step-indicator {
  background-color: #10b981;
  color: white;
}

.step-item.active .step-indicator {
  background-color: #428ED2;
  color: white;
}

.step-indicator {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background-color: #e5e7eb;
  color: #6b7280;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.75rem;
  font-weight: bold;
  margin-right: 0.75rem;
}

.step-content {
  flex: 1;
}

.step-title {
  font-weight: 500;
  color: #374151;
  font-size: 0.9rem;
}

.step-description {
  font-size: 0.8rem;
  color: #6b7280;
  margin-top: 0.25rem;
}

/* Error State */
.error-message {
  margin-top: 1rem;
  border: 1px dotted #E8362D;
  padding: 1rem;
}

/* Completion Actions */
.action-footer {
  display: flex;
  justify-content: center;
  margin-top: 2rem;
  padding-top: 1rem;
  margin-bottom: 1rem;
}

/* Installation Summary */
.installation-summary {
  margin: 1.5rem 0;
  padding: 1.5rem;
  border-radius: 8px;
  background-color: #f0f9ff;
  border: 1px solid #bfdbfe;
}

.summary-details {
  margin-top: 1rem;
  display: grid;
  gap: 0.5rem;
}

/* Virtual Scrolling Styles */
.log-container {
  text-align: left;
  background-color: white;
}

.log-virtual-container {
  height: 300px;
  overflow-y: auto;
  overflow-x: hidden;
  will-change: scroll-position;
  -webkit-overflow-scrolling: touch;
  scroll-behavior: smooth;
}

.virtual-spacer-top,
.virtual-spacer-bottom {
  width: 100%;
  pointer-events: none;
}

.log-scroll-container {
  contain: layout style;
}

.log-entry {
  height: 24px;
  display: flex;
  align-items: flex-start;
  contain: layout;
  box-sizing: border-box;
}

.log-message {
  margin: 0;
  padding: 2px 4px;
  font-family: monospace;
  font-size: 0.85rem;
  line-height: 20px;
  text-rendering: optimizeSpeed;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  width: 100%;
  flex: 1;
}

/* Log level styling */
.log-message.log-error {
  background-color: #fee2e2;
  color: #b91c1c;
  border-left: 3px solid #ef4444;
}

.log-message.log-warning {
  background-color: #fef3c7;
  color: #d97706;
  border-left: 3px solid #f59e0b;
}

.log-message.log-success {
  color: #059669;
  border-left: 3px solid #10b981;
}

.log-message.highlight {
  background-color: #fff9c2;
  font-weight: 500;
  border-left: 3px solid #E8362D;
}

.log-count {
  font-size: 0.8rem;
  color: #6b7280;
  font-weight: normal;
}

/* Scrollbar styling */
.log-virtual-container::-webkit-scrollbar {
  width: 8px;
}

.log-virtual-container::-webkit-scrollbar-track {
  background: #f1f1f1;
  border-radius: 4px;
}

.log-virtual-container::-webkit-scrollbar-thumb {
  background: #c1c1c1;
  border-radius: 4px;
}

.log-virtual-container::-webkit-scrollbar-thumb:hover {
  background: #a1a1a1;
}

/* Performance optimizations */
.log-virtual-container * {
  backface-visibility: hidden;
}

/* Button styling */
.n-button {
  color: #e5e7eb;
  background: #E8362D;
}

.n-button[type="primary"] {
  color: #e5e7eb;
  background-color: #E8362D;
}

.n-card {
  border: none;
  border-top: 1px solid #e5e7eb;
  padding-top: 0px;
}

.n-collapse {
  background-color: #FAFAFA;
  border: 1px solid #D5D5D5;
  max-height: 300px;
  overflow: auto;
}

/* Responsive adjustments */
@media (max-width: 768px) {
  .log-virtual-container {
    height: 250px;
  }

  .log-entry {
    height: 28px;
  }

  .log-message {
    line-height: 24px;
  }
}
</style>

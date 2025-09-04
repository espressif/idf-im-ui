<template>
  <div class="installation-progress" data-id="installation-progress">
    <h1 class="title" data-id="installation-title">
      {{ is_fix_mode ? 'Repair Progress' : 'Installation Progress' }}
    </h1>

    <n-alert title="Installation Error" type="error" v-if="error_message">
      {{ error_message }}
    </n-alert>

    <n-card class="progress-card" data-id="progress-card">
      <div class="summary-section" data-id="installation-summary"
        v-if="!installation_running && !installation_finished && !installation_failed">

        <!-- Fix Mode Summary -->
        <div v-if="is_fix_mode" class="fix-info" data-id="fix-info">
          <h3 data-id="fix-title">Repairing ESP-IDF Installation:</h3>
          <div class="fix-version-info">
            <div class="idf-version" v-if="fixing_version">
              {{ fixing_version.name || 'Unknown Version' }}
            </div>
            <div class="fix-path" v-if="fixing_version">
              <strong>Path:</strong> {{ fixing_version.path || 'Unknown Path' }}
            </div>
          </div>
          <div class="fix-description">
            <p>This will reinstall the selected ESP-IDF version and repair any corrupted or missing components.</p>
          </div>
        </div>

        <!-- Normal Install Mode Summary -->
        <div v-else class="versions-info" v-if="all_settings" data-id="versions-info">
          <h3 data-id="versions-title">Installing ESP-IDF Versions:</h3>
          <div class="version-chips" data-id="version-chips">
            <div v-for="version in idf_versions" :key="version" type="info" :data-id="`version-tag-${version}`"
              class="idf-version">
              {{ version }}
            </div>
          </div>
        </div>

        <!-- Start Installation Button - Only show when appropriate -->
        <div data-id="start-button-container" v-if="shouldShowStartButton()">
          <n-button @click="startInstallation()" type="error" size="large" :loading="installation_running"
            :disabled="installation_running" data-id="start-installation-button">
            {{ installation_running ? 'Installing...' : 'Start Installation' }}
          </n-button>
        </div>
      </div>

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
        <h3 data-id="error-title">Error during {{ is_fix_mode ? 'repair' : 'installation' }}:</h3>
        <p data-id="error-message-text">{{ error_message }} <br> For more information consult the log file.</p>
        <n-button @click="goHome()" type="error" size="large" data-id="home-installation-button">Go Back</n-button>
      </div>

      <!-- Completion Actions -->
      <div class="action-footer" v-if="installation_finished && !installation_failed" data-id="action-footer">
        <n-button @click="handleCompletion()" type="error" size="large" data-id="complete-installation-button-footer">
          {{ is_fix_mode ? 'Complete Repair' : 'Complete Installation' }}
        </n-button>
      </div>

      <!-- Installation/Repair Summary -->
      <div v-if="installation_finished && !installation_failed" class="installation-summary"
        data-id="installation-summary">
        <h3>{{ is_fix_mode ? 'Repair Complete' : 'Installation Complete' }}</h3>
        <p>{{ is_fix_mode ? 'Successfully repaired ESP-IDF installation and all required tools.' : 'Successfully installed ESP-IDF and all required tools.' }}</p>
        <div class="summary-details">
          <div v-if="installed_versions.length > 0">
            <strong>{{ is_fix_mode ? 'Repaired Version:' : 'Installed Versions:' }}</strong> {{ installed_versions.join(', ') }}
          </div>
          <div v-if="installationPath">
            <strong>Installation Path:</strong> {{ installationPath }}
          </div>
          <div v-if="completedToolsCount > 0">
            <strong>Tools {{ is_fix_mode ? 'Repaired' : 'Installed' }}:</strong> {{ completedToolsCount }}
          </div>
        </div>
      </div>

      <!-- Installation Log -->
      <n-collapse arrow-placement="right" v-if="totalLogCount > 0">
        <n-collapse-item title="Installation Log" name="1">
          <template #header-extra>
            <span class="log-count">({{ totalLogCount }} entries)</span>
            <!-- <n-button
              size="small"
              type="tertiary"
              @click.stop="clearLogs()"
              style="margin-left: 8px;"
            >
              Clear
            </n-button> -->
          </template>

          <div class="log-container">
            <!-- Virtual scrolling container -->
            <div
              class="log-virtual-container"
              ref="virtualContainer"
              @scroll="onLogScroll"
            >
              <!-- Spacer for items above viewport -->
              <div
                class="virtual-spacer-top"
                :style="{ height: topSpacerHeight + 'px' }"
              ></div>

              <!-- Only render visible items -->
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

              <!-- Spacer for items below viewport -->
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
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin, NCard, NTag, NTabs, NTabPane, NTable, NCollapse, NCollapseItem, NAlert, NProgress } from 'naive-ui'
import { listen } from '@tauri-apps/api/event'
import { useWizardStore } from '../../store'

export default {
  name: 'InstallationProgress',
  props: {
    nextstep: Function,
    mode: {
      type: String,
      default: 'install' // 'install' or 'fix'
    },
    fixVersionId: {
      type: String,
      default: null
    }
  },
  components: {
    NButton, NSpin, NCard, NTag, NTabs, NTabPane, NTable, NCollapse,
    NCollapseItem, NAlert, NProgress
  },

  data: () => ({
    os: undefined,
    all_settings: undefined,
    loading: true,
    tools: {},

    // Event listeners
    unlistenProgress: undefined,
    unlistenLog: undefined,

    // Installation state
    installation_running: false,
    installation_finished: false,
    installation_failed: false,
    error_message: "",

    // Progress tracking
    // currentProgress: 0,
    // currentActivity: "Preparing installation...",
    // currentDetail: "",
    currentStep: 0,
    currentStage: 'checking',

    // Version tracking
    current_version: null,
    installed_versions: [],
    failed_versions: [],

    // Installation steps
    installationSteps: [
      { title: 'Check', description: 'System requirements' },
      { title: 'Prerequisites', description: 'Installing dependencies' },
      { title: 'Download', description: 'Cloning repository' },
      { title: 'Submodules', description: 'Downloading submodules' },
      { title: 'Tools', description: 'Installing development tools' },
      { title: 'Python', description: 'Setting up Python environment' },
      { title: 'Configure', description: 'Finalizing configuration' },
      { title: 'Complete', description: 'Installation complete' }
    ],

    // Logging with Virtual scrolling
    visibleLogs: [], // Only visible log entries (reactive)
    totalLogCount: 0, // Total number of logs (reactive for UI)
    scrollTop: 0,
    containerHeight: 300, // Height of the scroll container
    itemHeight: 24, // Height of each log item in pixels
    visibleCount: 15, // Number of items to render
    startIndex: 0, // Starting index of visible items

    // UI state
    installationPath: "",
    completedToolsCount: 0,
    totalToolsCount: 0,
    showToolsTable: false,

    // progress tracking
    progressUpdateTrigger: 0,
    lastProgressUpdate: 0
  }),

  created() {
    this._allLogs = [];
    this.BUFFER_SIZE = 2; // Extra items to render for smooth scrolling

    this._progressData = {
      currentProgress: 0,
      currentActivity: "Preparing installation...",
      currentDetail: "",
      lastUpdate: Date.now()
    };

    // Throttle UI updates to prevent memory explosion
    this._progressThrottle = null;
  },

  methods: {
    goHome: function () {
      this.store.resetWizard();
      this.$router.push('/');
    },

    startInstallation: async function () {
      this.installation_running = true;
      this.installation_finished = false;
      this.installation_failed = false;
      this.error_message = "";
      this.log_messages = [];
      this.currentProgress = 0;

      try {
        if (this.is_fix_mode) {
          // For fix mode, the installation should already be started by the confirmFix call
          // Just ensure we're tracking the right version
          if (this.fixing_version) {
            this.current_version = this.fixing_version.name;
            this.currentActivity = `Repairing ${this.fixing_version.name}...`;
          }
        } else {
          // Normal installation
          await invoke("start_installation", {});
        }
      } catch (e) {
        console.error('Error during installation:', e);
        this.error_message = e.toString();
        this.installation_failed = true;
        this.installation_running = false;
      }
    },

    startListening: async function () {
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

    handleProgressEvent: function (payload) {
      const { stage, percentage, message, detail, version } = payload;
      const now = Date.now();

      // Update basic progress info
      // this.currentProgress = percentage || 0;
      // this.currentActivity = message || this.currentActivity;
      // this.currentDetail = detail || "";
      // this.currentStage = stage;

      // Store in non-reactive object (no memory leak)
      this._progressData.currentProgress = percentage || 0;
      this._progressData.currentActivity = message || this._progressData.currentActivity;
      this._progressData.currentDetail = detail || "";
      this._progressData.lastUpdate = now;

      // Update current version if provided
      if (version && version !== this.current_version) {
        this.current_version = version;
      }

      // Update stage (reactive, but changes rarely)
      if (stage !== this.currentStage) {
        this.currentStage = stage;
      }
      let newStep = this.currentStep;

      switch (stage) {
        case 'checking': newStep = 0; break;
        case 'prerequisites': newStep = 1; break;
        case 'download':
          newStep = 2;
          // Show submodules step when progress > 10%
          if (percentage > 10) newStep = 3;
          break;
        case 'extract': newStep = 3; break;
        case 'tools':
          newStep = 4;
          if (!this.showToolsTable) this.showToolsTable = true;
          break;
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

      // Throttle UI updates to max 2 per second to prevent memory explosion
      this.throttledProgressUpdate();
    },

    throttledProgressUpdate() {
      if (this._progressThrottle) {
        clearTimeout(this._progressThrottle);
      }

      this._progressThrottle = setTimeout(() => {
        const now = Date.now();
        // Only update if enough time has passed (100ms = 10fps)
        if (now - this.lastProgressUpdate > 100) {
          this.progressUpdateTrigger++;
          this.lastProgressUpdate = now;
        }
        this._progressThrottle = null;
      }, 100);
    },

    forceProgressUpdate() {
      this.progressUpdateTrigger++;
      this.lastProgressUpdate = Date.now();
    },

    handleLogMessage: function (payload) {
      const { level, message } = payload;

      // Create log entry (non-reactive)
      const logEntry = {
        level,
        text: message,
        timestamp: Date.now(),
        id: this._allLogs.length
      };

      // Add to non-reactive array
      this._allLogs.unshift(logEntry);

      // Keep reasonable limit
      const MAX_LOG_ENTRIES = 1000;
      if (this._allLogs.length > MAX_LOG_ENTRIES) {
        this._allLogs = this._allLogs.slice(0, MAX_LOG_ENTRIES);
      }

      // Update reactive counter
      this.totalLogCount = this._allLogs.length;

      // Update visible logs
      this.updateVisibleLogs();

      // Auto-scroll to top for new messages
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

      // Only update reactive array with visible subset
      this.visibleLogs = this._allLogs.slice(startIndex, endIndex).map(log => ({
        ...log
      }));
    },

    onLogScroll(event) {
      const newScrollTop = event.target.scrollTop;

      // Throttle updates for performance
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

    clearLogs: function() {
      this._allLogs = [];
      this.visibleLogs = [];
      this.totalLogCount = 0;
      this.scrollTop = 0;
      this.startIndex = 0;

      // Force update
      this.$forceUpdate();
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

    getLogMessageClass: function (message) {
      if (message.level === 'error') return 'log-message log-error';
      if (message.level === 'warning') return 'log-message log-warning';
      if (message.level === 'success') return 'log-message log-success';
      if (message.text && (message.text.includes('WARN') || message.text.includes('ERR'))) {
        return 'log-message highlight';
      }
      return 'log-message';
    },


    handleToolsProgress: function (message, detail, percentage) {
      if (!this.current_version) return;

      // Initialize tools tracking for current version if needed
      if (!this.tools[this.current_version]) {
        this.tools[this.current_version] = {};
      }

      // Extract tool name from message
      let toolName = null;
      if (message.includes('Installing:') || message.includes('Downloading:') || message.includes('Extracting:')) {
        const match = message.match(/(?:Installing:|Downloading:|Extracting:|Installed:)\s*(.+)/);
        if (match && match[1]) {
          toolName = match[1].trim();
        }
      }

      if (toolName) {
        // Initialize tool if not exists
        if (!this.tools[this.current_version][toolName]) {
          this.tools[this.current_version][toolName] = {
            displayName: toolName,
            status: 'pending',
            progress: 0
          };
          this.totalToolsCount++;
        }

        const tool = this.tools[this.current_version][toolName];

        // Update tool status based on message
        if (message.includes('Downloading:') || message.includes('Preparing:')) {
          tool.status = 'downloading';
        } else if (message.includes('Verifying:')) {
          tool.status = 'verifying';
        } else if (message.includes('Extracting:')) {
          tool.status = 'extracting';
        } else if (message.includes('Installed:')) {
          tool.status = 'completed';
          tool.progress = 100;
          this.completedToolsCount++;
        }

        // Extract progress from detail if available
        if (detail) {
          const progressMatch = detail.match(/(\d+)%/);
          if (progressMatch) {
            tool.progress = parseInt(progressMatch[1]);
          }
        }
      }
    },

    handleInstallationComplete: function (version) {
      this.installation_running = false;
      this.installation_finished = true;
      this.currentProgress = 100;

      if (version && !this.installed_versions.includes(version)) {
        this.installed_versions.push(version);
      }

      // If current_version is set but not in installed_versions, add it
      if (this.current_version && !this.installed_versions.includes(this.current_version)) {
        this.installed_versions.push(this.current_version);
      }
    },

    handleInstallationError: function (message, detail) {
      this.installation_running = false;
      this.installation_failed = true;
      this.error_message = message || "Installation failed";

      if (this.current_version && !this.failed_versions.includes(this.current_version)) {
        this.failed_versions.push(this.current_version);
      }
    },

    getToolStatusText: function (status) {
      const statusMap = {
        'pending': 'Waiting',
        'downloading': 'Downloading',
        'verifying': 'Verifying',
        'extracting': 'Extracting',
        'completed': 'Completed',
        'error': 'Error'
      };
      return statusMap[status] || status;
    },

    getToolStatusClass: function (status) {
      const classMap = {
        'pending': 'tool-status-pending',
        'downloading': 'tool-status-active',
        'verifying': 'tool-status-active',
        'extracting': 'tool-status-active',
        'completed': 'tool-status-success',
        'error': 'tool-status-error'
      };
      return classMap[status] || '';
    },

    getLogMessageClass: function (message) {
      // Cache class combinations to avoid repeated string concatenation
      if (message.level === 'error') {
        return 'log-message log-error';
      } else if (message.level === 'warning') {
        return 'log-message log-warning';
      } else if (message.level === 'success') {
        return 'log-message log-success';
      }

      // Check for highlights only if needed
      if (message.text.includes('WARN') || message.text.includes('ERR')) {
        return 'log-message highlight';
      }

      return 'log-message';
    },

    get_settings: async function () {
      this.all_settings = await invoke("get_settings", {});
      if (this.all_settings && this.all_settings.path) {
        this.installationPath = this.all_settings.path;
      }
    },

    get_os: async function () {
      this.os = await invoke("get_operating_system", {});
    },
    // Enhanced navigation handler
    handleCompletion() {
      if (this.nextstep && typeof this.nextstep === 'function') {
        // Called from wizard flow - use provided nextstep function
        this.nextstep();
      } else {
        // Called directly via router - handle navigation here
        this.handleDirectNavigation();
      }
    },

    handleDirectNavigation() {
      if (this.is_fix_mode) {
        // For fix mode, return to version management
        this.$router.push({
          path: '/version-management',
          query: {
            fixed: this.fixing_version?.id,
            message: `Successfully repaired ESP-IDF ${this.fixing_version?.name}`
          }
        });
      } else {
        // For direct installation, go to welcome with success message
        this.$router.push({
          path: '/welcome',
          query: {
            installed: 'true',
            versions: this.installed_versions.join(','),
            message: `Successfully installed ESP-IDF versions: ${this.installed_versions.join(', ')}`
          }
        });
      }
    },

    goHome() {
      // Handle home navigation consistently
      if (this.store && this.store.setStep) {
        // If we have wizard store, reset it
        this.store.setStep(1);
      }

      if (this.is_fix_mode) {
        // For fix mode, return to version management
        this.$router.push('/version-management');
      } else {
        // For normal installation, go to welcome
        this.$router.push('/welcome');
      }
    },

    // Enhanced start installation to handle both modes
    startInstallation: async function () {
      this.installation_running = true;
      this.installation_finished = false;
      this.installation_failed = false;
      this.error_message = "";
      this.log_messages = [];
      // this.currentProgress = 0;

      this.clearLogs();

      // Reset progress data
      this._progressData = {
        currentProgress: 0,
        currentActivity: "Preparing installation...",
        currentDetail: "",
        lastUpdate: Date.now()
      };

      this.currentStep = 0;
      this.currentStage = 'checking';
      this.forceProgressUpdate();

      try {
        if (this.is_fix_mode) {
          // For fix mode, the installation should already be started by the confirmFix call
          if (this.fixing_version) {
            this.current_version = this.fixing_version.name;
            // this.currentActivity = `Repairing ${this.fixing_version.name}...`;
            this._progressData.currentActivity = `Repairing ${this.fixing_version.name}...`;
          }
        } else {
          // Normal installation
          await invoke("start_installation", {});
        }
      } catch (e) {
        console.error('Error during installation:', e);
        this.error_message = e.toString();
        this.installation_failed = true;
        this.installation_running = false;
      }
    },

    // Add method to determine if we should show start button
    shouldShowStartButton() {
      // Show start button only if:
      // 1. Not in fix mode (fix starts automatically)
      // 2. Installation not running and not failed
      // 3. Not already finished
      return !this.is_fix_mode &&
            !this.installation_running &&
            !this.installation_failed &&
            !this.installation_finished;
    },

    cleanupProgressData() {
      if (this._progressThrottle) {
        clearTimeout(this._progressThrottle);
        this._progressThrottle = null;
      }
      this._progressData = null;
    },

    cleanup: function() {
      // Enhanced cleanup
      this.clearLogs();
      this.cleanupProgressData();

      // Remove event listeners
      window.removeEventListener('resize', this.measureContainer);

      if (this.unlistenProgress) {
        this.unlistenProgress();
        this.unlistenProgress = null;
      }
      if (this.unlistenLog) {
        this.unlistenLog();
        this.unlistenLog = null;
      }

      // Clean up non-reactive reference
      this._allLogs = null;
    },
  },



  computed: {
    store() {
      return useWizardStore()
    },

    idf_versions() {
      return this.all_settings ? this.all_settings.idf_versions : [];
    },

    tools_tabs() {
      return [...new Set([
        ...this.installed_versions,
        ...this.failed_versions,
        ...(this.current_version ? [this.current_version] : [])
      ])];
    },
    // Fix the fix mode detection
    is_fix_mode() {
      return this.mode === 'fix' || this.$route.query.mode === 'fix';
    },

    // Get the version being fixed from route params
    fixing_version() {
      if (this.is_fix_mode) {
        return {
          id: this.$route.query.id || this.fixVersionId,
          name: this.$route.query.name || 'Unknown Version',
          path: this.$route.query.path || 'Unknown Path'
        };
      }
      return null;
    },
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
      // Depend on the trigger to know when to update
      this.progressUpdateTrigger;
      return this._progressData ? this._progressData.currentProgress : 0;
    },

    currentActivity() {
      this.progressUpdateTrigger;
      return this._progressData ? this._progressData.currentActivity : "Preparing installation...";
    },

    currentDetail() {
      this.progressUpdateTrigger;
      return this._progressData ? this._progressData.currentDetail : "";
    }
  },

  mounted() {
    this.get_os();
    this.get_settings();
    this.startListening();
    this.measureContainer();
    window.addEventListener('resize', this.measureContainer);

    // Handle different entry modes
    if (this.is_fix_mode && this.$route.query.mode === 'fix') {
        // Fix mode - installation should already be started
        this.installation_running = true;
        if (this.fixing_version) {
          this.current_version = this.fixing_version.name;
          this.currentActivity = `Preparing to repair ${this.fixing_version.name}...`;
        }
      } else if (this.$route.query.autostart === 'true') {
        // Auto-start installation (e.g., from simple setup)
        this.startInstallation();
      }

      // Handle any success/error messages passed via route
      if (this.$route.query.message) {
        // You could show a toast notification here
        console.log('Route message:', this.$route.query.message);
    }
  },

  beforeDestroy() {
    this.cleanup();
  },
  beforeUnmount() {
    this.cleanup();
  },
}
</script>

<style scoped>
.installation-progress {
  padding: 2rem;
  max-width: 1000px;
  margin: 0 auto;
}

.progress-card {
  background: white;
  padding: 1.5rem;
  display: flex;
  flex-direction: column;
  align-content: center;
}

.summary-section {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
  margin-bottom: 2rem;
}

.versions-info h3 {
  font-size: 1.1rem;
  color: #374151;
  margin-bottom: 1rem;
}

.version-chips {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
}

.idf-version {
  border: 1px solid #428ED2;
  border-radius: 4px;
  width: 124px;
  height: 40px;
  display: flex;
  justify-content: center;
  align-items: center;
}

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

.tools-section {
  margin-top: 1rem;
}

.tools-tabs {
  margin-top: 1rem;
}

.tool-progress {
  font-weight: 500;
}

.tool-status-pending {
  color: #6b7280;
}

.tool-status-active {
  color: #428ED2;
  font-weight: 500;
}

.tool-status-success {
  color: #10b981;
  font-weight: 500;
}

.tool-status-error {
  color: #ef4444;
  font-weight: 500;
}

.error-message {
  margin-top: 1rem;
  border: 1px dotted #E8362D;
  padding: 1rem;
}

.action-footer {
  display: flex;
  justify-content: center;
  margin-top: 2rem;
  padding-top: 1rem;
  margin-bottom: 1rem;
}

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
  height: 300px; /* Fixed height for virtual scrolling */
  overflow-y: auto;
  overflow-x: hidden;
  /* Hardware acceleration for smooth scrolling */
  will-change: scroll-position;
  -webkit-overflow-scrolling: touch;
  /* Improve scrolling performance */
  scroll-behavior: smooth;
}

.virtual-spacer-top,
.virtual-spacer-bottom {
  /* Spacers to maintain scroll height */
  width: 100%;
  pointer-events: none;
}

.log-scroll-container {
  /* Container for visible items */
  contain: layout style;
}

.log-entry {
  /* Fixed height for consistent virtual scrolling */
  height: 24px; /* Must match itemHeight in data */
  display: flex;
  align-items: flex-start;
  contain: layout;
  /* Prevent layout thrashing */
  box-sizing: border-box;
}

.log-message {
  margin: 0;
  padding: 2px 4px;
  font-family: monospace;
  font-size: 0.85rem;
  line-height: 20px; /* Ensure consistent height */
  /* Optimize text rendering */
  text-rendering: optimizeSpeed;
  /* Prevent text wrapping to maintain height */
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  /* Full width */
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
  /* Reduce repaints */
  backface-visibility: hidden;
}

/* Responsive adjustments */
@media (max-width: 768px) {
  .log-virtual-container {
    height: 250px;
  }

  .log-entry {
    height: 28px; /* Slightly taller on mobile */
  }

  .log-message {
    line-height: 24px;
  }
}

/* Fix mode specific styles */
.fix-info {
  text-align: center;
}

.fix-info h3 {
  font-size: 1.1rem;
  color: #374151;
  margin-bottom: 1rem;
}

.fix-version-info {
  margin: 1rem 0;
}

.fix-path {
  margin-top: 0.5rem;
  font-size: 0.9rem;
  color: #6b7280;
}

.fix-description {
  margin-top: 1rem;
  padding: 1rem;
  background-color: #fffbeb;
  border: 1px solid #fbbf24;
  border-radius: 6px;
}

.fix-description p {
  margin: 0;
  color: #92400e;
  font-size: 0.9rem;
}

.n-button {
  background: #E8362D;
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

tbody span {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 20px;
  color: #428ED2
}

tr > td {
  text-align: center;
}

tr > td:first-child {
  text-align: left;
}

.n-tab-pane {
  max-height: 300px;
  overflow-y: auto;
}
</style>

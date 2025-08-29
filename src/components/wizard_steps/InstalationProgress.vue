<template>
  <div class="installation-progress" data-id="installation-progress">
    <h1 class="title" data-id="installation-title">Installation Progress</h1>
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

        <div data-id="start-button-container" v-if="!is_fix_mode">
          <n-button @click="startInstallation()" type="error" size="large" :loading="installation_running"
            :disabled="installation_running" data-id="start-installation-button" v-if="!installation_failed">
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

      <!-- Tools Progress Display -->
      <!-- <div v-if="tools_tabs.length > 0 && showToolsTable" class="tools-section" data-id="tools-section">
        <n-tabs type="card" class="tools-tabs" data-id="tools-tabs">
          <n-tab-pane v-for="version in tools_tabs" :key="version" :tab="version" :name="version"
            :data-id="`tools-tab-${version}`">
            <n-table striped data-id="tools-table">
              <thead>
                <tr data-id="tools-table-header">
                  <th>Tool</th>
                  <th>Status</th>
                  <th>Progress</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="(tool, name) in tools[version]" :key="name" :data-id="`tool-row-${version}-${name}`">
                  <td data-id="tool-name">{{ tool.displayName || name }}</td>
                  <td>
                    <span
                      :class="getToolStatusClass(tool.status)"
                      :data-id="`tool-status-${version}-${name}`"
                    >
                      {{ getToolStatusText(tool.status) }}
                    </span>
                  </td>
                  <td>
                    <div class="tool-progress">
                      {{ tool.progress || 0 }}%
                    </div>
                  </td>
                </tr>
              </tbody>
            </n-table>
          </n-tab-pane>
        </n-tabs>
      </div> -->

      <div v-if="installation_failed" class="error-message" data-id="error-message">
        <h3 data-id="error-title">Error during installation:</h3>
        <p data-id="error-message-text">{{ error_message }} <br> For more information consult the log file.</p>
        <n-button @click="goHome()" type="error" size="large" data-id="home-installation-button">Go Back</n-button>
      </div>

      <div class="action-footer" v-if="installation_finished && !installation_failed" data-id="action-footer">
        <n-button @click="nextstep" type="error" size="large" data-id="complete-installation-button-footer">
          Complete Installation
        </n-button>
      </div>

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

      <n-collapse arrow-placement="right" v-if="log_messages.length > 0">
        <n-collapse-item title="Installation Log" name="1">
          <div class="log-container">
            <pre v-for="(message, index) in log_messages" :key="index" class="log-message"
              :class="getLogMessageClass(message)">{{ message.text }}</pre>
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
    currentProgress: 0,
    currentActivity: "Preparing installation...",
    currentDetail: "",
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

    // Logging
    log_messages: [],

    // UI state
    installationPath: "",
    completedToolsCount: 0,
    totalToolsCount: 0,
    showToolsTable: false
  }),

  methods: {
    goHome: function () {
      this.store.setStep(1);
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
        console.log('Progress event received:', event.payload);
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

      // Update basic progress info
      this.currentProgress = percentage || 0;
      this.currentActivity = message || this.currentActivity;
      this.currentDetail = detail || "";
      this.currentStage = stage;

      // Update current version if provided
      if (version && version !== this.current_version) {
        this.current_version = version;
      }

      // Handle different stages
      switch (stage) {
        case 'checking':
          this.currentStep = 0;
          break;
        case 'prerequisites':
          this.currentStep = 1;
          break;
        case 'download':
          this.currentStep = 2;
          // Show submodules step when progress > 10%
          if (percentage > 10) {
            this.currentStep = 3;
          }
          break;
        case 'extract':
          this.currentStep = 3;
          break;
        case 'tools':
          this.currentStep = 4;
          this.showToolsTable = true;
          this.handleToolsProgress(message, detail, percentage);
          break;
        case 'python':
          this.currentStep = 5;
          break;
        case 'configure':
          this.currentStep = 6;
          break;
        case 'complete':
          this.currentStep = 7;
          this.handleInstallationComplete(version);
          break;
        case 'error':
          this.handleInstallationError(message, detail);
          break;
      }
    },

    handleLogMessage: function (payload) {
      const { level, message } = payload;
      this.log_messages.push({
        level,
        text: message,
        timestamp: new Date().toLocaleTimeString()
      });

      // Keep log size manageable
      if (this.log_messages.length > 1000) {
        this.log_messages = this.log_messages.slice(-800);
      }

      // Extract installation path from logs if available
      if (message.includes('installed at:') || message.includes('Location:')) {
        const pathMatch = message.match(/(?:installed at:|Location:)\s*(.+)/i);
        if (pathMatch && pathMatch[1]) {
          this.installationPath = pathMatch[1].trim();
        }
      }
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
      const classes = ['log-message'];

      if (message.level === 'error') {
        classes.push('log-error');
      } else if (message.level === 'warning') {
        classes.push('log-warning');
      } else if (message.level === 'success') {
        classes.push('log-success');
      }

      // Highlight important messages
      if (message.text.includes('WARN') || message.text.includes('ERR')) {
        classes.push('highlight');
      }

      return classes.join(' ');
    },

    get_settings: async function () {
      this.all_settings = await invoke("get_settings", {});
      if (this.all_settings && this.all_settings.path) {
        this.installationPath = this.all_settings.path;
      }
    },

    get_os: async function () {
      this.os = await invoke("get_operating_system", {});
    }
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
    }
  },

  mounted() {
    this.get_os();
    this.get_settings();
    this.startListening();
    // If we're in fix mode and coming from the router, start tracking immediately
    if (this.is_fix_mode && this.$route.query.mode === 'fix') {
      this.installation_running = true;
      if (this.fixing_version) {
        this.current_version = this.fixing_version.name;
        this.currentActivity = `Preparing to repair ${this.fixing_version.name}...`;
      }
    }
  },

  beforeDestroy() {
    if (this.unlistenProgress) this.unlistenProgress();
    if (this.unlistenLog) this.unlistenLog();
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

.log-container {
  text-align: left;
  background-color: white;
  max-height: 300px;
  overflow-y: auto;
}

.log-message {
  margin: 0;
  padding: 2px 0;
  font-family: monospace;
  font-size: 0.85rem;
}

.log-message.highlight {
  background-color: #fff9c2;
  font-weight: 500;
  border-left: 3px solid #E8362D;
  padding-left: 6px;
}

.log-message.log-error {
  background-color: #fee2e2;
  color: #b91c1c;
  padding: 4px;
  margin: 2px 0;
}

.log-message.log-warning {
  background-color: #fef3c7;
  color: #d97706;
  padding: 2px 4px;
}

.log-message.log-success {
  color: #059669;
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

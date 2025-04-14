<template>
  <div class="installation-progress" data-id="installation-progress">
    <h1 class="title" data-id="installation-title">Installation Progress</h1>
    <n-alert title="Installation Error" type="error" v-if="error_message">
      {{ error_message }}
    </n-alert>
    <n-card class="progress-card" data-id="progress-card">
      <div class="summary-section" data-id="installation-summary"
        v-if="!installation_running && !installation_finished && !installation_failed">
        <div class="versions-info" v-if="all_settings" data-id="versions-info">
          <h3 data-id="versions-title">Installing ESP-IDF Versions:</h3>
          <div class="version-chips" data-id="version-chips">
            <div v-for="version in idf_versions" :key="version" type="info" :data-id="`version-tag-${version}`"
              class="idf-version">
              {{ version }}
            </div>
          </div>
        </div>
        <div data-id="start-button-container">
          <n-button @click="startInstalation()" type="error" size="large" :loading="installation_running"
            :disabled="installation_running" data-id="start-installation-button" v-if="!installation_failed">
            {{ installation_running ? 'Installing...' : 'Start Installation' }}
          </n-button>
        </div>
      </div>

      <!-- Current Activity Display (Win only) -->
      <div v-if="installation_running && os == 'windows'" class="current-activity" data-id="current-activity">
        <div class="current-step">
          <h3>Current Activity:</h3>
          <div class="activity-status">{{ currentActivity }}</div>
        </div>

        <div class="progress-section">
          <div class="progress-label">Overall Progress</div>
          <n-progress type="line" :percentage="calculateOverallProgress" :processing="installation_running"
            :indicator-placement="'inside'" color="#E8362D" />
        </div>
      </div>

      <div v-if="tools_tabs.length > 0 && os != 'windows'" class="tools-section" data-id="tools-section">
        <n-tabs type="card" class="tools-tabs" data-id="tools-tabs">
          <n-tab-pane v-for="version in tools_tabs" :key="version" :tab="version" :name="version"
            :data-id="`tools-tab-${version}`">
            <n-table striped data-id="tools-table">
              <thead>
                <tr data-id="tools-table-header">
                  <th>Tool</th>
                  <th>Downloaded</th>
                  <th>SHA</th>
                  <th>Extracted</th>
                  <th>Finished</th>
                  <th>Error</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="(tool, name) in tools[version]" :key="name" :data-id="`tool-row-${version}-${name}`">
                  <td data-id="tool-name">{{ tool.name }}</td>
                  <td><span :type="tool.downloaded ? 'success' : 'default'"
                      :data-id="`tool-downloaded-${version}-${name}`">{{ tool.downloaded ? '✓' : '✗' }}</span></td>
                  <td><span :type="tool.verified ? 'success' : 'default'"
                      :data-id="`tool-verified-${version}-${name}`">{{ tool.verified ? '✓' : '✗' }}</span></td>
                  <td><span :type="tool.extracted ? 'success' : 'default'"
                      :data-id="`tool-extracted-${version}-${name}`">{{ tool.extracted ? '✓' : '✗' }}</span></td>
                  <td><span :type="tool.finished ? 'success' : 'default'"
                      :data-id="`tool-finished-${version}-${name}`">{{ tool.finished ? '✓' : '✗' }}</span></td>
                  <td><span :type="tool.error ? 'error' : 'default'" :data-id="`tool-error-${version}-${name}`">{{
                    tool.error ? '✓' : '✗' }}</span></td>
                </tr>
              </tbody>
            </n-table>
          </n-tab-pane>
        </n-tabs>
        <GlobalProgress messagePosition="right" v-if="!installation_finished && !installation_failed" />
        <div v-if="installation_failed" class="error-message" data-id="error-message">
          <h3 data-id="error-title">Error during installation:</h3>
          <p data-id="error-message-text">{{ error_message }} <br> For more information consult the log file.</p>
          <n-button @click="goHome()" type="error" size="large" data-id="home-installation-button">Go Back</n-button>
        </div>
      </div>


      <div class="action-footer" v-if="installation_finished && !installation_failed" data-id="action-footer">
        <n-button @click="nextstep" type="error" size="large" data-id="complete-installation-button-footer">
          Complete Installation
        </n-button>
      </div>

      <div v-if="installation_finished && !installation_failed" class="installation-summary"
        data-id="installation-summary">
        <h3>Installation Complete</h3>
        <p>Successfully installed ESP-IDF and all required tools.</p>
        <div class="summary-details">
          <div><strong>Installed Version:</strong> {{ curently_installing_version || versions_finished[0] }}</div>
          <div><strong>Installation Path:</strong> {{ installationPath }}</div>
          <div><strong>Tools Installed:</strong> {{ completedToolsCount }}</div>
        </div>
      </div>

      <n-collapse arrow-placement="right" v-if="new_install_messages.length > 0">
        <n-collapse-item title="Installation Log" name="1">
          <div class="log-container">
            <pre v-for="message in new_install_messages" :key="message" class="log-message"
              :class="{ 'highlight': isHighlightMessage(message) }">{{ message }}</pre>
          </div>
        </n-collapse-item>
      </n-collapse>
      <div class="error-log-container" v-if="new_install_error_messages.length > 0">
        <pre v-for="message in new_install_error_messages" :key="message" class="log-message error">{{ message }}</pre>
      </div>
    </n-card>
  </div>
</template>


<script>
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin, NCard, NTag, NTabs, NTabPane, NTable, NCollapse, NCollapseItem, NAlert, NProgress } from 'naive-ui'
import { listen } from '@tauri-apps/api/event'
import GlobalProgress from "./../GlobalProgress.vue";
import { useWizardStore } from '../../store'

export default {
  name: 'InstalationProgress',
  props: {
    nextstep: Function
  },
  components: {
    NButton, NSpin, NCard, NTag, NTabs, NTabPane, NTable, NCollapse,
    NCollapseItem, GlobalProgress, NAlert, NProgress
  },

  data: () => ({
    os: undefined,
    all_settings: undefined,
    loading: true,
    tools: {},
    unlisten: undefined,
    unlistenNew: undefined,
    unlistenTools: undefined,
    unlistenProgress: undefined,
    progressMessage: "",
    progressStatus: "info",
    progressPercentage: "0",
    progressDisplay_progress: true,
    installation_running: false,
    installation_finished: false,
    installation_failed: false,
    error_message: "",
    curently_installing_version: undefined,
    versions_finished: [],
    versions_failed: [],
    new_install_messages: [],
    new_install_error_messages: [],
    // New properties for improved UI
    currentActivity: "Preparing installation...",
    windowsToolStatus: [],
    installationPath: "",
    completedToolsCount: 0,
    totalTools: 0,
    highlightKeywords: [
      "WARN",
      "ERR"
    ]
  }),
  methods: {
    goHome: function () {
      this.store.setStep(1);
      this.$router.push('/');
    },
    startInstalation: async function () {
      this.installation_running = true;
      try {
        const _ = await invoke("start_installation", {});
      } catch (e) {
        console.error('Error during installation:', e);
        this.error_message = e;
        this.installation_failed = true;
      }
    },
    startListening: async function () {
      // windows
      this.unlistenNew = await listen('installation_output', (event) => {
        const { type, message } = event.payload;
        console.log('### Received new message:', message);

        if (type === 'stdout') {
          this.parseWindowsLogMessage(message);
          if (message.includes('DEBUG') || message.includes('TRACE')) {
            return; // ignore debug and trace messages
          }
          let parts = message.split(' - ');
          if (parts.length > 1) {
            parts.shift();
            this.new_install_messages.push(parts.join(' - '));
          } else {
            this.new_install_messages.push(message);
          }

        } else if (type === 'stderr') {
          this.new_install_error_messages.push(message);
        }
      });

      await listen('installation_complete', (event) => {
        const { success, message } = event.payload;
        if (success) {
          this.installation_running = false;
          this.installation_finished = true;
          this.progressMessage = message;
          this.progressStatus = "success";
          this.currentActivity = "Installation Complete";

          // Set summary data
          this.completedToolsCount = this.windowsToolStatus.filter(tool => tool.status === 'completed').length;
        } else {
          this.installation_running = false;
          this.installation_failed = true;
          this.error_message = message;
        }
      });

      // POSIX
      this.unlistenTools = await listen('tools-message', (event) => {
        console.log('### Received tools message:', event.payload);
        switch (event.payload.action) {
          case 'start':
            this.tools[this.curently_installing_version][event.payload.tool] = {
              name: event.payload.tool,
              started: true,
              downloaded: false,
              verified: false,
              extracted: false,
              error: false,
              finished: false,
            };
            break;
          case 'match':
            this.tools[this.curently_installing_version][event.payload.tool].finished = true;
            break;
          case 'downloaded':
            this.tools[this.curently_installing_version][event.payload.tool].downloaded = true;
            break;
          case 'extracted':
            this.tools[this.curently_installing_version][event.payload.tool].extracted = true;
            this.tools[this.curently_installing_version][event.payload.tool].finished = true;
            break;
          case 'error':
            this.tools[this.curently_installing_version][event.payload.tool].error = true;
            this.installation_running = false;
            this.installation_failed = true;
            break;
          case 'download_verified':
            this.tools[this.curently_installing_version][event.payload.tool].verified = true;
            break;
          case 'download_verification_failed':
            this.tools[this.curently_installing_version][event.payload.tool].verified = false;
            this.installation_running = false;
            this.installation_failed = true;
            break;
          default:
            console.warn('Unknown action:', event.payload.action);
        }
      });

      this.unlisten = await listen('install-progress-message', (event) => {
        switch (event.payload.state) {
          case 'started':
            this.tools[event.payload.version] = {};
            this.curently_installing_version = event.payload.version;
            this.installation_running = true;
            break;
          case 'finished':
            this.versions_finished.push(event.payload.version);
            this.curently_installing_version = undefined;
            this.installation_running = false;
            this.installation_finished = true;
            break;
          case 'failed':
            this.versions_failed.push(event.payload.version);
            this.curently_installing_version = undefined;
            this.installation_running = false;
            this.installation_failed = true;
            this.installation_finished = true;
            console.error('Error during installation:', event.payload.version);
            break;
          default:
            console.warn('Unknown state:', event.payload.state);
        }
      });
    },
    startListeningToInstalationProgress: async function () {
      console.log('Listening for progress messages...');
      this.progressDisplay_progress = true;
      this.unlistenProgress = await listen('progress-message', (event) => {
        this.progressMessage = event.payload.message;
        this.progressStatus = event.payload.status;
        this.progressPercentage = event.payload.percentage;
      })
      this.progressDisplay_progress = false;
    },
    get_settings: async function () {
      this.all_settings = await invoke("get_settings", {});
      if (this.all_settings && this.all_settings.path) {
        this.installationPath = this.all_settings.path;
      }
    },
    get_os: async function () {
      this.os = await invoke("get_operating_system", {});
      return false;
    },
    get_logs_path: async function () {
      this.LogPath = await invoke("get_logs_folder", {});
    },
    parseWindowsLogMessage: function (message) {
      if (message.includes('idf_path:')) {
        const pathMatch = message.match(/idf_path:\s*(.*)/);
        if (pathMatch && pathMatch[1]) {
          this.installationPath = pathMatch[1].trim();
        }
      }

      // Extract current activity
      if (message.includes('Checking for prerequisites')) {
        this.currentActivity = "Checking prerequisites...";
      } else if (message.includes('Python sanity check')) {
        this.currentActivity = "Verifying Python installation...";
      } else if (message.includes('Selected idf version:')) {
        const versionMatch = message.match(/Selected idf version:\s*\[(.*?)\]/);
        if (versionMatch && versionMatch[1]) {
          this.curently_installing_version = versionMatch[1].replace(/"/g, '');
          this.currentActivity = `Preparing to install ESP-IDF version ${this.curently_installing_version}`;
        }
      } else if (message.includes('Downloading tools:')) {
        const toolsMatch = message.match(/Downloading tools:\s*\[(.*?)\]/);
        if (toolsMatch && toolsMatch[1]) {
          const toolsList = toolsMatch[1].split(',').map(tool => tool.trim().replace(/"/g, ''));
          this.totalTools = toolsList.length;

          // Initialize tool status tracking
          this.windowsToolStatus = toolsList.map(name => ({
            name,
            status: 'pending',
            downloadProgress: 0
          }));

          this.currentActivity = `Downloading ${this.totalTools} tools...`;
        }
      } else if (message.includes('Downloading tool:')) {
        const toolMatch = message.match(/Downloading tool:\s*(.*)/);
        if (toolMatch && toolMatch[1]) {
          const toolName = toolMatch[1].trim();
          this.currentActivity = `Downloading ${toolName}...`;

          // Update tool status
          const toolIndex = this.windowsToolStatus.findIndex(t => t.name === toolName);
          if (toolIndex >= 0) {
            this.windowsToolStatus[toolIndex].status = 'downloading';
          }
        }
      } else if (message.includes('Decompressing')) {
        const toolMatch = message.match(/Decompressing.*?\/(.*?)\.zip/);
        if (toolMatch && toolMatch[1]) {
          const toolName = toolMatch[1];
          this.currentActivity = `Extracting ${toolName}...`;

          // Update tool status
          const toolIndex = this.windowsToolStatus.findIndex(t =>
            t.name === toolName || toolMatch[1].includes(t.name));
          if (toolIndex >= 0) {
            this.windowsToolStatus[toolIndex].status = 'extracting';
          }
        }
      } else if (message.includes('extracted tool:')) {
        const toolMatch = message.match(/extracted tool:\s*(.*)/);
        if (toolMatch && toolMatch[1]) {
          const filename = toolMatch[1].trim();
          const toolName = filename;

          // Update tool status
          const toolIndex = this.windowsToolStatus.findIndex(t =>
            t.name === toolName || filename.includes(t.name));
          if (toolIndex >= 0) {
            this.windowsToolStatus[toolIndex].status = 'completed';
            this.completedToolsCount++;
          }
        }
      } else if (message.includes('Successfully installed IDF')) {
        this.currentActivity = "Installation complete";
        this.installation_running = false;
        this.installation_finished = true;
      }
    },

    isHighlightMessage: function (message) {
      return this.highlightKeywords.some(keyword => message.includes(keyword));
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
      return this.versions_finished.concat(this.versions_failed).concat(this.curently_installing_version ? [this.curently_installing_version] : []);
    },
    calculateOverallProgress() {
      if (!this.installation_running) {
        return this.installation_finished ? 100 : 0;
      }

      if (this.windowsToolStatus.length === 0) {
        return 10; // Initial stage
      }

      const completed = this.windowsToolStatus.filter(t => t.status === 'completed').length;
      const extracting = this.windowsToolStatus.filter(t => t.status === 'extracting').length;
      const downloading = this.windowsToolStatus.filter(t => t.status === 'downloading').length;

      // Weight: completed tools count most, extracting count as half-done
      const progress = (completed + (extracting * 0.5) + (downloading * 0.25)) / this.totalTools * 100;

      return Math.min(Math.max(Math.round(progress), 10), 99); // Keep between 10-99% during installation
    }
  },
  mounted() {
    this.get_os();
    this.get_settings();
    this.startListening();
    this.startListeningToInstalationProgress();
    this.get_logs_path();
  },
  beforeDestroy() {
    if (this.unlisten) this.unlisten();
    if (this.unlistenTools) this.unlistenTools();
    if (this.unlistenProgress) this.unlistenProgress();
    if (this.unlistenNew) this.unlistenNew();
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

.status-grid {
  display: grid;
  gap: 1.5rem;
}

.status-item {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.status-label {
  font-size: 0.875rem;
  color: #6b7280;
}

.tools-section {
  margin-top: 1rem;
}

.tools-tabs {
  margin-top: 1rem;
}

.action-footer {
  display: flex;
  justify-content: center;
  margin-top: 2rem;
  padding-top: 1rem;
  margin-bottom: 1rem;
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

.n-button {
  background: #E8362D;
}

.n-card {
  border: none;
  border-top: 1px solid #e5e7eb;
  padding-top: 0px;
}

.n-card__content {
  padding-top: 0px;
}

tbody span {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 20px;
  color: #428ED2
}

tr>td {
  text-align: center;
}

tr>td:first-child {
  text-align: left;
}

.n-tab-pane {
  max-height: 300px;
  overflow-y: auto;
}

.progress-container {
  width: 75%;
  margin: auto;
  margin-top: 20px;
}

.progress-content {
  display: flex;
  vertical-align: middle;
  justify-content: center;
}

.error-message {
  margin-top: 1rem;
  border: 1px dotted #E8362D;
  padding: 1rem;
}

.n-collapse {
  background-color: #FAFAFA;
  border: 1px solid #D5D5D5;
  max-height: 300px;
  overflow: auto;
}

.n-collapse-item__header-main {
  display: flex;
  align-items: center;
}

.log-container {
  text-align: left;
  background-color: white;
}

/* New styles */
.current-activity {
  margin: 1rem 0;
  padding: 1rem;
  background-color: #f9fafb;
  border-radius: 8px;
  border-left: 4px solid #428ED2;
}

.activity-status {
  font-size: 1.1rem;
  font-weight: 500;
  margin-top: 0.5rem;
  color: #374151;
}

.progress-section {
  margin-top: 1rem;
}

.progress-label {
  font-size: 0.875rem;
  color: #6b7280;
  margin-bottom: 0.5rem;
}

.tools-progress {
  margin: 1.5rem 0;
}

/* Tool card styles removed as requested */

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

.log-message.highlight {
  background-color: #fff9c2;
  font-weight: 500;
  border-left: 3px solid #E8362D;
  padding-left: 6px;
}

.log-message.error {
  background-color: #fee2e2;
  color: #b91c1c;
  padding: 4px;
  margin: 2px 0;
}

.error-log-container {
  margin-top: 1rem;
  padding: 0.5rem;
  border: 1px solid #fecaca;
  border-radius: 6px;
  background-color: #fef2f2;
  max-height: 200px;
  overflow-y: auto;
}

.current-step {
  display: flex;
  flex-direction: column;
}

.current-step h3 {
  margin: 0;
  font-size: 1rem;
  color: #6b7280;
}
</style>
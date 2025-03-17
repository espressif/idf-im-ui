<template>
  <div class="installation-progress" data-id="installation-progress">
    <h1 class="title" data-id="installation-title">Installation Progress</h1>

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


      <div v-if="tools_tabs.length > 0" class="tools-section" data-id="tools-section">
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
    </n-card>
  </div>
</template>


<script>
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin, NCard, NTag, NTabs, NTabPane, NTable } from 'naive-ui'
import { listen } from '@tauri-apps/api/event'
import GlobalProgress from "./../GlobalProgress.vue";
import { useWizardStore } from '../../store'

export default {
  name: 'InstalationProgress',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin, NCard, NTag, NTabs, NTabPane, NTable, GlobalProgress },

  data: () => ({
    os: undefined,
    all_settings: undefined,
    loading: true,
    tools: {},
    unlisten: undefined,
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
  }),
  methods: {
    goHome: function () {
      this.store.setStep(1);
      this.$router.push('/');
    },
    startInstalation: function () {
      this.installation_running = true;
      console.log('### Starting installation...');
      const result = invoke("start_installation", {});
      result.then((data) => {
        console.log('### Installation finished with result:', data);
        this.installation_finished = true;
        this.installation_failed = false;
      }).catch((e) => {
        console.error('Error during installation:', e);
        this.error_message = e;
        this.installation_failed = true;
      }).finally(() => {
        console.log('### Installation finished');
        this.installation_running = false;
      });
    },
    startListening: function () {
      listen('tools-message', (event) => {
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
      }).then((handler) => {
        console.log('### Listening for tools messages...');
        this.unlistenTools = handler;
      }).catch((e) => {
        console.error('Error listening for tools messages:', e);
      });
      listen('install-progress-message', (event) => {
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
            break;
          case 'failed':
            this.versions_failed.push(event.payload.version);
            this.curently_installing_version = undefined;
            this.installation_running = false;
            this.installation_failed = true;
            console.error('Error during installation:', event.payload.version);
            break;
          default:
            console.warn('Unknown state:', event.payload.state);
        }
      }).then((handler) => {
        console.log('### Listening for tools messages...');
        this.unlisten = handler;
      }).catch((e) => {
        console.error('Error listening for tools messages:', e);
      });
    },
    startListeningToInstalationProgress: async function () {
      console.log('Listening for progress messages...');
      this.progressDisplay_progress = true;
      listen('progress-message', (event) => {
        this.progressMessage = event.payload.message;
        this.progressStatus = event.payload.status;
        this.progressPercentage = event.payload.percentage;
      }).then((handler) => {
        console.log('### Listening for progress messages...');
        this.unlistenProgress = handler;
      }).catch((e) => {
        console.error('Error listening for progress messages:', e);
      });
      this.progressDisplay_progress = false;
    },
    get_settings: function () {
      invoke("get_settings", {}).then((settings) => {
        console.info('Got settings:', settings);
        this.all_settings = settings;
      }).catch((e) => {
        console.error('Error getting settings:', e);
      });
    },
    get_os: async function () {
      invoke("get_operating_system", {}).then((os) => {
        console.info('Got OS:', os);
        this.os = os;
      }).catch((e) => {
        console.error('Error getting OS:', e);
      });
      return false;
    },
    get_logs_path: async function () {
      invoke("get_logs_folder", {}).then((LogPath) => {
        console.info('Got logs path:', LogPath);
        this.LogPath = LogPath;
      }).catch((e) => {
        console.error('Error getting logs path:', e);
      });
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
      return this.versions_finished.concat(this.versions_failed).concat(this.curently_installing_version ? this.curently_installing_version : []);
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
  /* border-top: 1px solid #e5e7eb; */
  margin-top: -2.5rem;

}

.tools-tabs {
  margin-top: 1rem;
}

.action-footer {
  display: flex;
  justify-content: center;
  margin-top: 2rem;
  padding-top: 1rem;
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
  padding-top: 0xp;
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

.n-progress {
  width: 50%;
  margin-top: 6px;
  margin-right: 6px;
}

.error-message {
  margin-top: 1rem;
  border: 1px dotted #E8362D;
  padding: 1rem;
}
</style>
<template>
  <div class="installation-progress" data-id="installation-progress">
    <h1 class="title" data-id="installation-title">Installation Progress</h1>

    <n-card class="progress-card" data-id="progress-card">
      <div class="summary-section" data-id="installation-summary" v-if="!instalation_running && !instalation_finished">
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
          <n-button @click="startInstalation()" type="error" size="large" :loading="instalation_running"
            :disabled="instalation_running" data-id="start-installation-button">
            {{ instalation_running ? 'Installing...' : 'Start Installation' }}
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
        <GlobalProgress messagePosition="right" v-if="!instalation_finished" />
      </div>

      <div class="action-footer" v-if="instalation_finished" data-id="action-footer">
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
    instalation_running: false,
    instalation_finished: false,
    curently_installing_version: undefined,
    versions_finished: [],
    versions_failed: [],
  }),
  methods: {
    startInstalation: async function () {
      this.instalation_running = true;
      const _ = await invoke("start_installation", {});
      this.instalation_running = false;
      this.instalation_finished = true;
    },
    startListening: async function () {
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

            break;
          case 'download_verified':
            this.tools[this.curently_installing_version][event.payload.tool].verified = true;
            break;
          case 'download_verification_failed':
            this.tools[this.curently_installing_version][event.payload.tool].verified = false;
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
            break;
          case 'finished':
            this.versions_finished.push(event.payload.version);
            this.curently_installing_version = undefined;
            break;
          case 'failed':
            this.versions_failed.push(event.payload.version);
            this.curently_installing_version = undefined;
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
      this.all_settings = await invoke("get_settings", {});;
    },
    get_os: async function () {
      this.os = await invoke("get_operating_system", {});;
      return false;
    },
  },
  computed: {
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
</style>
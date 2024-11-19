<template>
  <p>Instalation:</p>
  <p v-if="all_settings">The following versions will be installed:<span>{{ idf_versions.join(", ") }}</span></p>
  <n-button @click="startInstalation()" :disabled="instalation_running">Start Installation</n-button>
  <n-spin size="large" v-if="instalation_running" />
  <n-button @click="nextstep" type="primary" v-if="instalation_finished">Finish</n-button>
  <hr>
  <p v-if="!!curently_installing_version">Now installing: <span>{{ curently_installing_version }}</span></p>
  <p v-if="versions_finished.length > 0">Finished: <span>{{ versions_finished.join(", ") }}</span></p>
  <p v-if="versions_failed.length > 0">Failed: <span>{{ versions_failed.join(", ") }}</span></p>

  <hr>
  <n-tabs type="card" animated tab-style="min-width: 120px;" v-if="tools_tabs.length > 0">
    <n-tab-pane v-for="tab in tools_tabs" :key="tab" :tab="tab" :name="tab">
      <div v-if="tools[tab]">
        <p>Tools:</p>
        <table>
          <thead>
            <tr>
              <th>Name</th>
              <th>Downloaded</th>
              <th>Extracted</th>
              <th>Finished</th>
              <th>Error</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="tool in tools[tab]">
              <td>
                {{ tool.name }}
              </td>
              <td>
                {{ tool.downloaded ? 'yes' : 'no' }}
              </td>
              <td>
                {{ tool.extracted ? 'yes' : 'no' }}
              </td>
              <td>
                {{ tool.finished ? 'yes' : 'no' }}
              </td>
              <td>
                {{ tool.error ? 'yes' : 'no' }}
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </n-tab-pane>
  </n-tabs>
  <hr>
  <n-button @click="nextstep" type="primary" v-if="instalation_finished">Finish</n-button>
</template>

<script>
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin } from 'naive-ui'
import { listen } from '@tauri-apps/api/event'


export default {
  name: 'InstalationProgress',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin },

  data: () => ({
    os: undefined,
    all_settings: undefined,
    loading: true,
    tools: {},
    unlisten: undefined,
    unlistenTools: undefined,
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
      console.log('### Installation Finished ###');
      this.instalation_finished = true;
      return false;
    },
    startListening: async function () {
      console.log('Listening for tools messages...');
      this.unlistenTools = await listen('tools-message', (event) => {
        switch (event.payload.action) {
          case 'start':
            this.tools[this.curently_installing_version][event.payload.tool] = {
              name: event.payload.tool,
              started: true,
              downloaded: false,
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
  },
  beforeDestroy() {
    if (this.unlisten) {
      this.unlisten();
    }
    if (this.unlistenTools) {
      this.unlistenTools();
    }
  }

}
</script>

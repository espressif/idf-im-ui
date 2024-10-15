<template>
  <p>Instalation:</p>
  <p v-if="all_settings">The following versions will be installed:<span>{{ idf_versions.join(", ") }}</span></p>
  <n-button @click="startInstalation()">Start Instalation</n-button>
  <hr>
  <div v-if="tools">
    <p>Tools:</p>
    <table>
      <tr>
        <th>Name</th>
        <th>Downloaded</th>
        <th>Extracted</th>
        <th>Finished</th>
        <th>Error</th>
      </tr>
      <tr v-for="tool in tools">
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
    </table>
  </div>
  <hr>
</template>

<script>
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin } from 'naive-ui'
import loading from "naive-ui/es/_internal/loading";
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
  }),
  methods: {
    startInstalation: async () => {
      const _ = invoke("start_installation", {});
      console.log('start instalation ended!!!')
      return false;
    },
    startListening() {
      console.log('Listening for tools messages...');
      listen('tools-message', (event) => {
        // console.log('Received progress message:', event);
        switch (event.payload.action) {
          case 'start':
            this.tools[event.payload.tool] = {
              name: event.payload.tool,
              started: true,
              downloaded: false,
              extracted: false,
              error: false,
              finished: false,
            };
            break;
          case 'match':
            this.tools[event.payload.tool].finished = true;
            break;
          case 'downloaded':
            this.tools[event.payload.tool].downloaded = true;

            break;
          case 'extracted':
            this.tools[event.payload.tool].extracted = true;
            this.tools[event.payload.tool].finished = true;

            break;
          case 'error':
            this.tools[event.payload.tool].error = true;

            break;
          default:
            console.warn('Unknown action:', event.payload.action);
        }

      });
    },
    get_settings: async function () {
      this.all_settings = await invoke("get_settings", {});;
      return false;
    },
    get_os: async function () {
      this.os = await invoke("get_operating_system", {});;
      return false;
    },
  },
  computed: {
    idf_versions() {
      return this.all_settings.idf_versions;
    }
  },
  mounted() {
    this.get_os();
    this.get_settings();
    this.startListening();
  }
}
</script>

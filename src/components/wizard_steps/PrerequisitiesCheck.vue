<template>
  <p>Wizard will now check for the IDF Prerequisites...</p>
  <n-space vertical>
    <n-spin :show="loading">
      <n-alert title="List of needed Prerequisities" type="default">
        <ul>
          <li v-for="p in display_prerequisities" :key="p.name">{{ p.icon }} III {{ p.name }}</li>
        </ul>
      </n-alert>
      <template #description>
        Loading list of prerequisites...
      </template>
    </n-spin>
    <n-button @click="check_prerequisites" type="primary">Check Prerequisites</n-button>
    <n-divider />
    <div v-if="did_the_check_run && missing_prerequisities.length == 0"> <!-- if no missing prerequisities -->
      <p>Prerequisites check passed. You can now continue to next step.</p>
      <n-button @click="nextstep" type="primary">Next</n-button>
    </div>
    <div v-if="did_the_check_run && missing_prerequisities.length != 0"> <!-- Some missing prerequisities -->
      <p>The following prerequisites are missing:</p>
      <ul>
        <li v-for="p in missing_prerequisities" :key="p">{{ p }}</li>
      </ul>
      <div v-if="os == 'windows'">
        <p> The installer can attempt to install the missing prerequisites.</p>
        <p> If you want the installer to install the prerequisites, click on the install button.</p>
        <n-button @click="install_prerequisites" type="warning">Install Prerequisites</n-button>
        <n-spin :show="installing_prerequisities">
          <template #description>
            Installing prerequisites...
          </template>
        </n-spin>
      </div>
      <p v-else>Please install the missing prerequisites and rerun the check.</p>
    </div>
  </n-space>
</template>

<script>
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin } from 'naive-ui'
import loading from "naive-ui/es/_internal/loading";

export default {
  name: 'PrerequisitiesCheck',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin },
  data: () => ({
    loading: false,
    installing_prerequisities: false,
    did_the_check_run: false,
    all_prerequisities: [],
    missing_prerequisities: [],
    display_prerequisities: [],
    os: undefined,
  }),
  methods: {
    get_prerequisities_list: async function () {
      this.loading = true;
      this.all_prerequisities = await invoke("get_prequisites", {});;
      this.loading = false;
      this.display_prerequisities = this.all_prerequisities.map(p => ({
        name: p,
        icon: '❓',
      }));
      return false;
    },
    check_prerequisites: async function () {
      this.loading = true;
      let missing_list = await invoke("check_prequisites", {});
      this.missing_prerequisities = missing_list;
      console.log("missing prerequisities: ", missing_list);
      this.display_prerequisities = this.display_prerequisities.map(p => ({
        name: p.name,
        icon: missing_list.includes(p.name) ? '❌' : '✔',
      }));
      this.did_the_check_run = true;
      this.loading = false;
      return false;
    },
    install_prerequisites: async function () {
      this.installing_prerequisities = true;
      await invoke("install_prerequisites", {});
      this.check_prerequisites();
      this.installing_prerequisities = false;
      return false;
    },
    get_os: async function () {
      this.os = await invoke("get_operating_system", {});;
      return false;
    },
  },
  mounted() {
    this.get_prerequisities_list();
    this.get_os();
  }
}
</script>
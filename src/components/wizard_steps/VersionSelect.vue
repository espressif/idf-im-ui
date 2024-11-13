<template>
  <p>Please select IDF versions you want to install:</p>
  <n-space vertical>
    <n-spin :show="loading">
      <template #default>
        <ul>
          <li v-for="version in versions" :key="version">
            <n-checkbox v-model:checked="version.selected">
              {{ version.name }}
            </n-checkbox>
          </li>
        </ul>
      </template>
      <template #description>
        loading avalible IDF versions...
      </template>
    </n-spin>

    <n-button @click="processVersions" type="primary">Next</n-button>
  </n-space>
</template>

<script>
import { ref, version } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin } from 'naive-ui'
import loading from "naive-ui/es/_internal/loading";

export default {
  name: 'VersionSelect',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin },
  data: () => ({
    loading: true,
    versions: [],
  }),
  methods: {
    get_available_versions: async function () {
      const versions = await invoke("get_idf_versions", {});
      this.versions = versions;
      this.loading = false;
      return false;
    },
    processVersions: async function () {
      const selected_versions = this.versions.filter(version => version.selected).map(version => version.name);
      // todo: send to backend
      const _ = await invoke("set_versions", { versions: selected_versions });
      this.nextstep();
    }
  },
  mounted() {
    this.get_available_versions();
  }
}
</script>

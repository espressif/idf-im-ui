<template>
  <p>Instalation:</p>
  <p v-if="all_settings">The following versions will be installed:<span>{{ idf_versions.join(", ") }}</span></p>
  <n-button @click="startInstalation()">Start Instalation</n-button>
</template>

<script>
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin } from 'naive-ui'
import loading from "naive-ui/es/_internal/loading";

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
  }),
  methods: {
    startInstalation: async function () {
      return false;
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
  }
}
</script>

<template>
  <p>Wizard will now check your Python instalation...</p>
  <n-space vertical>
    <n-spin :show="loading">
      <p v-if=python_sane>Your Python meets the requirements!</p>
      <p v-else>Python is not sane. Please install Python 3.10 or later with pip, virtualenv and support for ssl.</p>
      <template #description>
        checking Python sanity...
      </template>
    </n-spin>
    <div v-if="!python_sane && !loading && os == 'Windows'">
      <p> The installer can attempt to install and setup Python for you.</p>
      <n-button @click="install_prerequisites" type="warning">Install Prerequisites</n-button>
    </div>
    <n-button v-if="python_sane" @click="nextstep" type="primary">Next</n-button>
  </n-space>
</template>

<script>
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin } from 'naive-ui'
import loading from "naive-ui/es/_internal/loading";

export default {
  name: 'PythonSanitycheck',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin },
  data: () => ({
    os: undefined,
    loading: true,
    python_sane: false
  }),
  methods: {
    check_python_sanity: async function () {
      this.python_sane = await invoke("python_sanity_check", {});;
      this.loading = false;
      return false;
    },
    get_os: async function () {
      this.os = await invoke("get_operating_system", {});;
      return false;
    },
  },
  mounted() {
    this.check_python_sanity();
    this.get_os();
  }
}
</script>

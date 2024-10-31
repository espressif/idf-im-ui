<template>
  <p>Wizard will now check your Python instalation on your system.</p>
  <n-space vertical>
    <n-spin :show="loading">
      <p v-if=python_sane>Your Python meets the requirements!</p>
      <p v-else>Python is not sane. Please install Python 3.10 or later with pip, virtualenv and support for ssl.</p>
      <template #description>
        checking Python sanity...
      </template>
    </n-spin>
    <n-spin :show="installing_python == true">
      <div v-if="!python_sane && os == 'windows'">
        <p> The installer can attempt to install and setup Python for you.</p>
        <n-button @click="install_python" type="warning">Install Python</n-button>
      </div>
      <p v-if="installing_python">Installing Python...</p>
    </n-spin>
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
    python_sane: false,
    installing_python: false,
  }),
  methods: {
    check_python_sanity: async function () {
      this.loading = true;
      this.python_sane = await invoke("python_sanity_check", {});;
      this.loading = false;
      return false;
    },
    get_os: async function () {
      this.os = await invoke("get_operating_system", {});
      this.os = this.os.toLowerCase();
      return false;
    },
    install_python: async function () {
      this.installing_python = true;
      await invoke("python_install", {});
      this.installing_python = false;
      this.check_python_sanity();
      return false;
    },
  },
  mounted() {
    this.check_python_sanity();
    this.get_os();
  }
}
</script>

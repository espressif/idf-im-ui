<template>
  <h1>Instalation was complete.</h1>
  <div v-if="os == 'windows'">
    <p>The installer placed icon on your desktop. You can use this icon to open IDF powershell</p>
  </div>
  <p>Thank you for using EIM.</p>
  <p>You can now save instalation config, if you want to reproduce the instalation on another machine.</p>
  <n-button @click="save_config" type="info">Save Config</n-button>
  <n-button @click="quit" type="error">Quit</n-button>
</template>

<script>
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin } from 'naive-ui'
import { save } from '@tauri-apps/plugin-dialog';
import loading from "naive-ui/es/_internal/loading";

export default {
  name: 'Complete',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin },
  data: () => ({
    os: undefined,
    loading: true,
  }),
  methods: {
    get_os: async function () {
      this.os = await invoke("get_operating_system", {});
      this.os = this.os.toLowerCase();
      return false;
    },
    save_config: async () => {
      const selected = await save({
        filters: [
          {
            name: 'eim_config',
            extensions: ['toml'],
          },
        ],
      });
      if (selected) {
        const _ = await invoke("save_config", { path: selected });
        console.log("Config saved to", selected);
      } else {
        // todo: emit message to user that config was not saved
        console.log("Config not saved");
      }
    },
    quit() {
      const _ = invoke("quit_app", {});
    },
  },
  mounted() {
    this.get_os();
  }
}
</script>

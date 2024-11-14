<template>
  <div class="welcome">
    <h1>Please select starting point!</h1>
    <n-split direction="horizontal" style="height: 450px" :max="0.75" :min="0.25">
      <template #1>
        <div :style="{ height: '450px' }">
          ( TODO: put opemn config icon here )
          <n-button @click="load_config" type="primary" ghost>
            Load instalation config.
          </n-button>
          <pre v-if="Object.keys(rust_settings).length > 0">{{ JSON.stringify(rust_settings, null, 2) }}</pre>
        </div>
      </template>
      <template #2>
        <div :style="{ height: '200px' }">

          ( TODO: put mighty wizard icon here )
          <n-button @click="startWizard" type="primary" ghost>
            Start Wizard.
          </n-button>
        </div>
      </template>
    </n-split>
  </div>
</template>

<script>
import { open } from '@tauri-apps/plugin-dialog';
import { NSplit, NButton } from 'naive-ui'
import { invoke } from "@tauri-apps/api/core";

export default {
  name: 'LoadConfig',
  components: { NSplit, NButton },
  data: () => ({
    rust_settings: {}
  }),
  methods: {
    startWizard() {
      this.$router.push('/wizard/1');
    },
    async gs() {
      this.rust_settings = await invoke("get_settings", {});
      return false;
    },
    async load_config() {
      console.log('Loading config...');
      const file = await open({
        title: 'Select installation config file',
        multiple: false,
        directory: false,
        filters: [
          { name: '*', extensions: ['toml'] },
        ],
      });
      const _ = await invoke("load_settings", { path: file });
      this.gs();
    }
  },
  mounted() {
    this.gs();
  }
}
</script>

<style scoped>
.welcome {
  text-align: center;
  padding: 20px;
  margin-bottom: 30px;
  border: 1px 0px 1px 0px solid #ccc;
  border-radius: 10px;
  box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);
}
</style>
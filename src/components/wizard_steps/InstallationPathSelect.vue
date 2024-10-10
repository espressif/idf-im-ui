<template>
  <p>Please select the installation folder:</p>
  <n-space vertical>
    <n-input-group>
      <n-input v-model:value="installPath" placeholder="Installation path" />
      <n-button @click="openFolderDialog" type="primary">Browse</n-button>
    </n-input-group>
    <n-button @click="processInstallPath" type="primary">Next</n-button>
  </n-space>
</template>

<script>
import { ref, onMounted } from 'vue';
import { invoke } from "@tauri-apps/api/core";
import { open } from '@tauri-apps/plugin-dialog';
import { homeDir } from '@tauri-apps/api/path';
import { NButton, NInput, NInputGroup, NSpace } from 'naive-ui';
import { path } from '@tauri-apps/api';

export default {
  name: 'InstallPathSelect',
  props: {
    nextstep: Function
  },
  components: { NButton, NInput, NInputGroup, NSpace },
  setup(props) {
    const installPath = ref('');

    onMounted(async () => {
      const homePath = await homeDir();
      installPath.value = await path.join(homePath, '.espressif');
    });

    const openFolderDialog = async () => {
      const selected = await open({
        directory: true,
        multiple: false,
      });
      if (selected) {
        installPath.value = await path.join(selected, '.espressif');
      }
    };

    const processInstallPath = async () => {
      // Here you would typically send the selected path to the backend
      // For now, we'll just log it and move to the next step
      console.log("Selected installation path:", installPath.value);
      // You might want to add a Tauri command to save this path
      await invoke("set_installation_path", { path: installPath.value });
      props.nextstep();
    };

    return {
      installPath,
      openFolderDialog,
      processInstallPath
    };
  }
}
</script>

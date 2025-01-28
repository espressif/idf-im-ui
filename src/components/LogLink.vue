<script>
import { NButton } from 'naive-ui'
import { invoke } from "@tauri-apps/api/core";

export default {
  name: 'LogLink',
  components: { NButton },
  data() {
    return {
      LogPath: ''
    }
  },
  async created() {
    this.LogPath = await invoke("get_logs_folder", {});
  },
  methods: {
    open_logs() {
      console.log("Opening logs folder: " + this.LogPath);
      window.open(this.LogPath, '_blank');
    }
  }
}
</script>
<template>
  <p>
    <a href="#" @click="open_logs($event)">Logs folder</a>
  </p>
</template>

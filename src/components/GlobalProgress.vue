<template>
  <div class="progress-display" v-if="display_progress">
    <p class="caption">{{ message }}</p>
    <n-progress type="line" :status="status" :percentage="percentage" indicator-placement="inside"
      :show-indicator="false" height="30" border-radius="2px" fill-border-radius="2px"></n-progress>
  </div>
</template>

<script>
import { NProgress } from 'naive-ui'
import { defineComponent, h } from 'vue'
import { listen } from '@tauri-apps/api/event'

export default {
  name: 'GlobalProgress',
  components: { NProgress },
  data: () => ({
    percentage: "0",
    status: "info",
    message: "",
    targets: [],
    display_progress: false,
  }),
  methods: {
    startListening() {
      listen('progress-message', (event) => {
        console.log('Received progress message:', event);
        this.message = event.payload.message || "";
        this.status = event.payload.status || "info";
        this.percentage = event.payload.percentage || 0;
        this.display_progress = event.payload.display || false;

      })
    }
  },
  mounted() {
    this.get_avalible_targets();
  }
}

</script>

<style scoped>
.progress-display {
  position: fixed;
  background-color: #9e9a9a;
  bottom: 0px;
  left: 0px;
  width: 100%;
  /* height: 60px; */
  z-index: 999;
}

.caption {
  color: white;
  font-size: 18px;
  font-weight: bold;
  margin: 10px;
  text-align: center;
}
</style>
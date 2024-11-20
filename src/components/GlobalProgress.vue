<template>
  <transition name="slide">
    <div class="progress-container" v-if="display_progress">
      <div class="progress-content">
        <span class="progress-message">{{ message }}</span>
        <n-progress type="line" :color="themeVars.errorColor" :status="status" :percentage="percentage" :height="36"
          :show-indicator="true" indicator-placement="inside" class="progress-bar" processing>
          <template #indicator>
            {{ percentage }}%
          </template>
        </n-progress>
      </div>
    </div>
  </transition>
</template>

<script>
import { NProgress } from 'naive-ui'
import { listen } from '@tauri-apps/api/event'
import { useThemeVars } from "naive-ui";


export default {
  name: 'GlobalProgress',
  components: { NProgress },
  data: () => ({
    percentage: "0",
    status: "info",
    message: "",
    targets: [],
    display_progress: false,
    unlisten: undefined,
    themeVars: useThemeVars()
  }),
  methods: {
    startListening: async function () {
      console.log('Listening for progress messages...');
      this.unlisten = await listen('progress-message', (event) => {
        // console.log('Received progress message:', event);
        this.message = event.payload.message;
        this.status = event.payload.status;
        this.percentage = event.payload.percentage;
        this.display_progress = event.payload.display;

      })
    }
  },
  mounted() {
    this.startListening();
  },
  beforeDestroy() {
    if (this.unlisten) {
      this.unlisten();
    }
  }
}

</script>

<style scoped>
.progress-container {
  position: fixed;
  bottom: 0;
  left: 0;
  width: 100%;
  background: rgba(0, 0, 0, 0.85);
  backdrop-filter: blur(4px);
  z-index: 999;
  padding: 1rem;
}

.progress-content {
  max-width: 800px;
  margin: 0 auto;
}

.progress-message {
  display: block;
  color: white;
  font-size: 1rem;
  font-weight: 500;
  margin-bottom: 0.75rem;
  text-align: center;
}

.progress-bar {
  background: rgba(255, 255, 255, 0.1);
}

.slide-enter-active,
.slide-leave-active {
  transition: transform 0.3s ease-in-out;
}

.slide-enter-from,
.slide-leave-to {
  transform: translateY(100%);
}
</style>
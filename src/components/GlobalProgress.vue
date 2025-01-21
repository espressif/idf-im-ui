<template>
  <div class="progress-container">
    <div class="progress-content">
      <span v-if="messagePosition === 'left'" class="progress-message">{{ message }}</span>
      <n-progress type="line" color="#E8362D" :status="status" :percentage="percentage" :height="16"
        :show-indicator="false" class="progress-bar" processing v-if="message.length > 0">
        <template #indicator>
          {{ percentage }}%
        </template>
      </n-progress>
      <n-spin size="small" v-else stroke="#E8362D" />
      <span v-if="messagePosition === 'right'" class="progress-message">{{ message }}</span>
    </div>
  </div>
</template>

<script>
import { NProgress } from 'naive-ui'
import { listen } from '@tauri-apps/api/event'
import { useThemeVars } from "naive-ui";


export default {
  name: 'GlobalProgress',
  components: { NProgress },
  props: {
    messagePosition: {
      type: String,
      default: 'left',
      validator: (value) => ['left', 'right'].includes(value)
    }
  },
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
  width: 75%;
  margin: auto;
}

.progress-content {
  display: flex;
  vertical-align: middle;
  justify-content: center;
}

.n-progress {
  width: 50%;
  margin-top: 6px;
  margin-right: 6px;
  ;
}

.progress-message {
  display: block;
  font-size: 1rem;
  font-weight: 500;
  margin-right: 5px;
  margin-left: 5px;
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
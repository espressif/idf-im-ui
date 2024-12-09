<template>
  <div class="simple-setup">
    <n-card class="status-card">
      <div class="status-content">
        <!-- Installation Progress Spinner -->
        <div v-if="isInProgress" class="progress-spinner">
          <n-spin size="large">
            <div class="spinner-content">
              <img src="../assets/espressif_logo.svg" alt="ESP-IDF Logo" />
              <div class="spinner-text">
                <h3>{{ getCurrentStateMessage }}</h3>
                <p>Please wait while the installation progresses...</p>
              </div>
            </div>
          </n-spin>
        </div>

        <!-- Status Messages -->
        <div v-if="showStatusMessage" class="status-message">
          <n-result :status="getCurrentStateStatus" :title="getCurrentStateTitle"
            :description="getCurrentStateDescription">
            <template #footer>
              <div class="action-buttons">
                <n-button v-if="showRetryButton" @click="startInstalation" type="error">
                  Try Again
                </n-button>
                <n-button v-if="showExpertButton" @click="$router.push('/wizard/1')" type="info">
                  Expert Mode
                </n-button>
              </div>
            </template>
          </n-result>
        </div>

        <!-- Complete Component -->
        <div v-if="current_state_code === 11">
          <Complete />
        </div>

        <!-- Installation Log -->
        <n-collapse v-if="messages.length > 0">
          <n-collapse-item title="Installation Log" name="1">
            <div class="log-container">
              <pre v-for="message in messages" :key="message" class="log-message">{{ message }}</pre>
            </div>
          </n-collapse-item>
        </n-collapse>
      </div>
    </n-card>
  </div>
</template>

<script>
import { NProgress, NSpin, NCard, NButton, NResult, NCollapse, NCollapseItem } from 'naive-ui'
import { listen } from '@tauri-apps/api/event'
import { invoke } from "@tauri-apps/api/core";
import Complete from './wizard_steps/Complete.vue';

export default {
  name: 'SimpleSetup',
  components: {
    Complete, NProgress, NSpin, NCard, NButton, NResult, NCollapse, NCollapseItem
  },
  data: () => ({
    messages: [],
    current_state_code: 0,
    unlisten: undefined,
    user_message_unlisten: undefined
  }),
  computed: {
    isInProgress() {
      return [1, 2, 5, 8, 10].includes(this.current_state_code);
    },
    showStatusMessage() {
      return [3, 4, 6, 7, 9, 12].includes(this.current_state_code);
    },
    showRetryButton() {
      return [3, 4, 6, 7, 12].includes(this.current_state_code);
    },
    showExpertButton() {
      return [3, 4, 6, 7, 9, 12].includes(this.current_state_code);
    },
    getCurrentStateStatus() {
      switch (this.current_state_code) {
        case 11:
          return 'success';
        case 1:
        case 2:
        case 5:
        case 8:
        case 10:
          return 'info';
        default:
          return 'error';
      }
    },
    getCurrentStateMessage() {
      const messages = {
        1: 'Starting Installation...',
        2: 'Installing Prerequisites...',
        5: 'Setting up Python...',
        8: 'Getting ESP-IDF Versions...',
        10: 'Installing ESP-IDF...'
      };
      return messages[this.current_state_code] || '';
    },
    getCurrentStateTitle() {
      const titles = {
        3: 'Prerequisites Installation Failed',
        4: 'Missing Prerequisites',
        6: 'Python Setup Failed',
        7: 'Python Version Not Found',
        9: 'Version Selection Failed',
        12: 'Installation Failed'
      };
      return titles[this.current_state_code] || '';
    },
    getCurrentStateDescription() {
      if ([3, 4].includes(this.current_state_code)) {
        return `Missing prerequisites: ${this.messages[this.messages.length - 1]}`;
      }
      if ([6, 7].includes(this.current_state_code)) {
        return 'Please install Python 3.10 or later with pip, venv and SSL support';
      }
      if (this.current_state_code === 9) {
        return 'Please use expert mode to manually select ESP-IDF version';
      }
      return 'Please try again or switch to expert mode for more control';
    }
  },
  methods: {
    async startListening() {
      this.unlisten = await listen('simple-setup-message', (event) => {
        console.log(event.payload);
        this.messages.push(event.payload.message);
        this.current_state_code = event.payload.code;
      });
      await this.startInstalation();
    },
    startInstalation: async function () {
      const listener = listen('user-message', (event) => {
        this.messages.push(event.payload.message);
      });
      await invoke("start_simple_setup", {});
      if (listener) {
        listener();
      };
    },
  },
  mounted() {
    this.startListening();
  },
  beforeUnmount() {
    if (this.unlisten) {
      this.unlisten();
    }
  }
}
</script>

<style scoped>
.simple-setup {
  padding: 2rem;
  max-width: 800px;
  margin: 0 auto;
}

.status-card {
  background: white;
}

.status-content {
  display: flex;
  flex-direction: column;
  gap: 2rem;
}

.progress-spinner {
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 200px;
  background: rgba(220, 38, 38, 0.03);
  border-radius: 8px;
  padding: 2rem;
}

.spinner-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1.5rem;
  text-align: center;
}

.spinner-text h3 {
  color: #374151;
  font-size: 1.25rem;
  margin-bottom: 0.5rem;
}

.spinner-text p {
  color: #6b7280;
}

.status-message {
  padding: 1rem;
}

.action-buttons {
  display: flex;
  gap: 1rem;
  justify-content: center;
}

.log-container {
  max-height: 300px;
  overflow-y: auto;
  background: #f3f4f6;
  border-radius: 0.375rem;
  padding: 1rem;
}

.log-message {
  margin: 0;
  padding: 0.25rem 0;
  font-family: monospace;
  font-size: 0.875rem;
  color: #374151;
  white-space: pre-wrap;
}
</style>
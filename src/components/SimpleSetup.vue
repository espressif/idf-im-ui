<template>
  <div class="simple-setup">
    <div class="wizard-header">
      <h1 class="header-title">Simplified Mode</h1>
    </div>
    <n-card class="status-card">
      <div class="status-content">
        <!-- Complete Component -->
        <div v-if="current_state_code === 11">
          <Complete />
        </div>

        <!-- Status Messages -->
        <div class="status-message" v-else>
          <div class="status-message-header">
            <n-icon :size="32" :class="getCurrentStateStatus" class="icon">
              <ExclamationCircleFilled />
            </n-icon>
            <h1 class="title" data-id="target-select-title">{{ getCurrentStateTitle }}</h1>
          </div>
          <div class="status-message-body">
            <p class="description" data-id="target-select-description">{{ getCurrentStateDescription }}</p>
          </div>
          <div :class="['user-message', 'user-message-' + last_user_message_type]" v-if="last_user_message.length > 0">
            {{ last_user_message }}
          </div>
          <div class="action-buttons">
            <n-button v-if="showRetryButton" @click="startInstalation" ghost type="error">
              Try Again
            </n-button>
            <n-button v-if="showExpertButton" @click="$router.push('/wizard/1')" type="info">
              Expert Mode
            </n-button>
          </div>
        </div>



        <GlobalProgress messagePosition="left" v-if="current_state_code > 0 && current_state_code < 11" />

        <!-- Installation Log -->
        <n-collapse arrow-placement="right" v-if="messages.length > 0">
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
import GlobalProgress from './GlobalProgress.vue';
import { ExclamationCircleFilled } from '@vicons/antd'

export default {
  name: 'SimpleSetup',
  components: {
    Complete, NProgress, NSpin, NCard, NButton, NResult, NCollapse, NCollapseItem, ExclamationCircleFilled, GlobalProgress
  },
  data: () => ({
    messages: [],
    current_state_code: 0,
    last_user_message: '',
    last_user_message_type: 0,
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
    getCurentClass() {
      return "simple_install_result_negative";
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
        1: 'Starting Installation...',
        2: 'Installing Prerequisites...',
        3: 'Prerequisites Installation Failed',
        4: 'Missing Prerequisites',
        5: 'Python Setup...',
        6: 'Python Setup Failed',
        7: 'Python Version Not Found',
        8: 'Getting ESP-IDF Versions...',
        9: 'Version Selection Failed',
        10: 'Installing ESP-IDF...',
        11: 'Installation Complete',
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
      if (this.getCurrentStateStatus === 'error') {
        return 'Please try again or switch to expert mode for more control';
      } else {
        return '';
      }
    }
  },
  methods: {
    async startListening() {
      this.unlisten = await listen('simple-setup-message', (event) => {
        console.log(event.payload);
        this.messages.push(event.payload.message);
        this.current_state_code = event.payload.code;
      });
      // await this.startInstalation();
    },
    startInstalation: async function () {
      const listener = await listen('user-message', (event) => {
        this.last_user_message = event.payload.message;
        this.last_user_message_type = event.payload.type;
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
    this.startInstalation();
  },
  beforeUnmount() {
    if (this.unlisten) {
      this.unlisten();
    }
  }
}
</script>

<style scoped>
.header-title {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 36px;
  font-weight: 500;
  color: #111827;
}

.simple-setup {
  padding: 2rem;
  max-width: 1440px;
  margin: 0 auto;
  margin-left: 80px;
  margin-right: 80px;
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

.n-collapse {
  background-color: #FAFAFA;
  border: 1px solid #D5D5D5;
}

.n-collapse-item__header-main {
  display: flex;
  align-items: center;
}

.log-container {
  text-align: left;
  background-color: white;
}

.title {
  font-size: 27px;
}

.n-icon {
  margin-top: 10px;
  margin-right: 16px;
}

.n-icon.error {
  color: #E8362D;
}

.n-icon.success {
  color: #5AC8FA;
}

.n-icon.info {
  color: #5AC8FA;
}

.status-message-header {
  width: 100%;
  display: flex;
  vertical-align: middle;
  justify-content: center;
  align-items: center;
}

.status-message-body {
  width: 100%;
  display: flex;
  vertical-align: middle;
  justify-content: center;
  align-items: center;
}

.user-message {
  margin-left: 20%;
  margin-right: 20%;
  margin-bottom: 10px;
  padding: 10px;
}

.user-message-error {
  background-color: #fdeae8;
  border-left: 4px solid #E8362D;
}

.user-message-info {
  background-color: #eaf3fb;
  border-left: 4px solid #5AC8FA;
}
</style>
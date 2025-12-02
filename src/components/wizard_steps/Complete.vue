<template>
  <div class="complete-screen" data-id="complete-screen">
    <n-result class="complete-result" status="success" :title="t('complete.title')"
      :description="t('complete.description')" data-id="completion-result">
      <template #footer>
        <div class="actions" data-id="completion-actions">
          <div class="info-section" data-id="info-section">
            <div v-if="os === 'windows'" class="windows-info" data-id="windows-info">
              <n-alert type="info" data-id="powershell-shortcut-alert">
                <template #icon>
                  <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" data-id="shortcut-icon">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                      d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
                  </svg>
                </template>
                {{ t('complete.messages.powershellShortcut') }}
              </n-alert>
            </div>


          </div>

          <div class="buttons" data-id="action-buttons">
            <n-button @click="save_config" type="info" class="save-button" dashed data-id="save-config-button">
              {{ t('complete.buttons.saveConfiguration') }}
            </n-button>
            <n-button @click="goHome" class="save-button" type="info" dashed data-id="home-button">
              {{ t('complete.buttons.home') }}
            </n-button>
            <n-button @click="quit" class="save-button" type="info" dashed data-id="exit-button">
              {{ t('complete.buttons.exitInstaller') }}
            </n-button>
          </div>
          <div class="config-save" data-id="config-save-section">
            <n-alert type="info" data-id="save-config-alert">
              <template #icon>
                <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" data-id="save-icon">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                    d="M8 7H5a2 2 0 00-2 2v9a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-3m-1 4l-3 3m0 0l-3-3m3 3V4" />
                </svg>
              </template>
              {{ t('complete.messages.saveConfigInfo') }}
            </n-alert>
          </div>
          <div class="post-install" data-id="post-install-section">
            <n-alert type="info" data-id="post-install-alert">
              <template #icon>
                <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" data-id="terminal-icon">
                  <path d="M6 9a.5.5 0 0 1 .5-.5h3a.5.5 0 0 1 0 1h-3A.5.5 0 0 1 6 9zM3.854 4.146a.5.5 0 1 0-.708.708L4.793 6.5 3.146 8.146a.5.5 0 1 0 .708.708l2-2a.5.5 0 0 0 0-.708l-2-2z"/>
                  <path d="M2 1a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V3a2 2 0 0 0-2-2H2zm12 1a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H2a1 1 0 0 1-1-1V3a1 1 0 0 1 1-1h12z"/>
                </svg>
              </template>
              <span v-if="os == 'windows'">{{ t('complete.messages.postInstallInstruction') }}</span>
              <span v-else>{{ t('complete.messages.postInstallInstructionPosix') }}</span>
            </n-alert>
          </div>
        </div>
      </template>
    </n-result>
  </div>
</template>


<script>
import { ref } from "vue";
import { useI18n } from 'vue-i18n';
import { invoke } from "@tauri-apps/api/core";
import { NButton, NResult, NAlert } from 'naive-ui'
import { save } from '@tauri-apps/plugin-dialog';
import loading from "naive-ui/es/_internal/loading";
import { useRouter } from 'vue-router'
import { useWizardStore, useAppStore } from '../../store'


export default {
  name: 'Complete',
  props: {
    nextstep: Function
  },
  components: { NButton, NResult, NAlert },
  setup() {
    const { t } = useI18n()
    return { t }
  },
  data: () => ({
    os: undefined,
    router: useRouter()
  }),
  computed: {
    wizStore() {
      return useWizardStore()
    },
    appStore() {
      return useAppStore()
    }
  },
  methods: {
    async get_os() {
      this.os = (await this.appStore.getOs()).toLowerCase();
    },
    goHome() {
      this.wizStore.resetWizard();
      this.router.push('/');
    },
    async save_config() {
      const selected = await save({
        title: this.t('complete.dialog.saveConfigTitle'),
        defaultPath: '/tmp/eim_config.toml',
        filters: [
          {
            name: this.t('complete.dialog.configFileName'),
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

<style scoped>
.complete-screen {
  text-align: center;
  padding: 2rem;
  max-width: 800px;
  margin: 0 auto;
}

.complete-result {
  margin: 0 auto;
  max-width: 800px;
}

.actions {
  display: flex;
  flex-direction: column;
  gap: 2rem;
}

.info-section {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.icon {
  width: 1.5rem;
  height: 1.5rem;
}

.buttons {
  display: flex;
  gap: 1rem;
  justify-content: center;
}

.n-button {
  background-color: white;
}

.n-button.exit-button {
  background-color: #1290d8;
}

.save-button {
  min-width: 160px;
}

.n-result {
  display: flex;
  flex-wrap: wrap;
  width: 100%;
  justify-content: center;
  align-items: center;
}

/* First row containing icon and header */
.n-result-icon,
.n-result-header {
  display: flex;
  align-items: center;
  /* vertical centering */
  justify-content: center;
  /* horizontal centering */
}

.n-result-icon {
  flex: 0 0 auto;
  width: 50px;
  /* or whatever width you need */
}

.n-result-icon svg {
  width: 40px;
  color: #E8362D;
}

.home-button {
  color: #E8362D;
}

.n-result-header {
  flex: 1;
  /* takes remaining space */
}

/* Footer row */
.n-result-footer {
  flex: 0 0 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}

/* .exit-button {
  color: black;
  background-color: #1290d8;
  border-color: #5AC8FA;
}

.exit-button:hover {
  color: black;
  background-color: #429acd;
  border-color: #5AC8FA;
} */
</style>


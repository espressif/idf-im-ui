<template>
  <div class="complete-screen">
    <n-result status="success" title="Installation Complete!"
      description="ESP-IDF has been successfully installed on your system">
      <template #footer>
        <div class="actions">
          <div class="info-section">
            <div v-if="os === 'windows'" class="windows-info">
              <n-alert type="info">
                <template #icon>
                  <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                      d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
                  </svg>
                </template>
                An IDF PowerShell shortcut has been created on your desktop
              </n-alert>
            </div>

            <div class="config-save">
              <n-alert type="info">
                <template #icon>
                  <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                      d="M8 7H5a2 2 0 00-2 2v9a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-3m-1 4l-3 3m0 0l-3-3m3 3V4" />
                  </svg>
                </template>
                Save your configuration to reproduce this installation on other machines
              </n-alert>
            </div>
          </div>

          <div class="buttons">
            <n-button @click="save_config" type="info" class="save-button">
              Save Configuration
            </n-button>
            <n-button @click="quit" type="error">
              Exit Installer
            </n-button>
          </div>
        </div>
      </template>
    </n-result>
  </div>
</template>


<script>
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NButton, NResult, NAlert } from 'naive-ui'
import { save } from '@tauri-apps/plugin-dialog';
import loading from "naive-ui/es/_internal/loading";

export default {
  name: 'Complete',
  props: {
    nextstep: Function
  },
  components: { NButton, NResult, NAlert },
  data: () => ({
    os: undefined,
  }),
  methods: {
    async get_os() {
      this.os = (await invoke("get_operating_system", {})).toLowerCase();
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

<style scoped>
.complete-screen {
  padding: 2rem;
  max-width: 800px;
  margin: 0 auto;
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

.save-button {
  min-width: 160px;
}
</style>
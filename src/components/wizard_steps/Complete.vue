<template>
  <div class="complete-screen" data-id="complete-screen">
    <n-result class="complete-result" status="success" title="Installation Complete!"
      description="ESP-IDF has been successfully installed on your system" data-id="completion-result">
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
                An IDF PowerShell shortcut has been created on your desktop
              </n-alert>
            </div>


          </div>

          <div class="buttons" data-id="action-buttons">
            <n-button @click="save_config" type="info" class="save-button" dashed data-id="save-config-button">
              Save Configuration
            </n-button>
            <n-button @click="quit" class="exit-button" type="info" data-id="exit-button">
              Exit Installer
            </n-button>
            <n-button @click="forceQuit" class="exit-button" type="primary" data-id="force-quit-button">
              Force Quit
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
              Save your configuration to reproduce this installation on other machines
            </n-alert>
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
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

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
    get_os() {
      invoke("get_operating_system", {}).then((os) => {
        console.info('Got OS:', os);
        this.os = os.toLowerCase();
      }).catch((e) => {
        console.error('Error getting OS:', e);
      });
    },
    async forceQuit() {
      console.log("Force quit button clicked");
      const window = WebviewWindow.getByLabel('main');
      window.then(async (window) => {
        await window.close();
      }).catch(async (error) => {
        console.error("Could not find or close main window:", error);
        // Fallback to process.exit
        await invoke("quit_app", {});
      }).finally(() => {
        console.log("Force quitting app");
        process.exit(0);
      });
    },
    save_config() {
      try {
        let defaultPath;
        if (this.os === 'windows') {
          defaultPath = 'C:\\Users\\Public\\eim_config.toml';
        } else {
          defaultPath = '/tmp/eim_config.toml';
        }
        console.log("Opening save dialog with default path:", defaultPath);
        save({
          title: 'Save installation config file',
          defaultPath,
          filters: [
            {
              name: 'eim_config.toml',
              extensions: ['toml'],
            },
          ],
        }).then((selected) => {
          console.log("Save dialog result:", selected);
          if (selected) {
            invoke("save_config", { path: selected }).then(() => {
              console.log("Config saved to", selected);
            }).catch((error) => {
              console.error("Error saving config:", error);
            });
          } else {
            console.log("Config not saved");
          }
        }).catch((error) => {
          console.error("Error opening save dialog:", error);
        });
      } catch (error) {
        console.error("Error saving config:", error);
      }
    },
    quit() {
      console.log("Exit button clicked");
      invoke("quit_app", {}).then(() => {
        console.log("App quit");
      }).catch((error) => {
        console.error("Error quitting app:", error);
      });
    }
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
  color: red;
  ;
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
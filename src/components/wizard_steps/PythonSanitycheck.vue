<template>
  <div class="python-check" data-id="python-check">
    <n-card class="status-card" data-id="python-status-card">
      <n-spin :show="loading" data-id="python-check-spinner">
        <div v-if="!loading" class="status-content" data-id="python-status-content">
          <n-result :status="python_sane ? 'success' : 'warning'"
            :title="python_sane ? t('pythonSanitycheck.status.ready.title') : t('pythonSanitycheck.status.setupRequired.title')"
            :description="python_sane ? t('pythonSanitycheck.status.ready.description') : t('pythonSanitycheck.status.setupRequired.description')"
            data-id="python-check-result">
            <template #footer>
              <CheckResultsList :items="checkResults" data-id="python-check-list" />
              <div class="action-buttons" data-id="python-action-buttons">
                <div v-if="!python_sane && os === 'windows'" class="install-section" data-id="python-install-section">
                  <n-button @click="install_python" type="warning" :loading="installing_python" :disabled="loading"
                    data-id="install-python-button">
                    {{ installing_python ? t('pythonSanitycheck.actions.installingPython') : t('pythonSanitycheck.actions.installPython') }}
                  </n-button>
                  <p class="install-note" data-id="install-python-note">{{ t('pythonSanitycheck.installNote') }}</p>
                </div>
                <n-button v-if="python_sane" @click="nextstep" type="error" :disabled="loading"
                  data-id="continue-button">
                  {{ t('pythonSanitycheck.actions.continueNext') }}
                </n-button>
                <n-button v-if="!python_sane" @click="check_python_sanity" type="error" data-id="recheck-python-button">
                  {{ t('pythonSanitycheck.actions.recheckInstallation') }}
                </n-button>
              </div>
            </template>
          </n-result>

          <div v-if="!python_sane && os !== 'windows' && checkResults.length === 0" class="manual-instructions"
            data-id="manual-install-instructions">
            <h3 data-id="manual-install-title">{{ t('pythonSanitycheck.manualInstall.title') }}</h3>
            <p data-id="manual-install-intro">{{ t('pythonSanitycheck.manualInstall.intro') }}</p>
            <ul data-id="manual-install-requirements">
              <li data-id="python-requirement">{{ t('pythonSanitycheck.manualInstall.requirements.python') }}</li>
              <li data-id="pip-requirement">{{ t('pythonSanitycheck.manualInstall.requirements.pip') }}</li>
              <li data-id="virtualenv-requirement">{{ t('pythonSanitycheck.manualInstall.requirements.virtualenv') }}</li>
              <li data-id="ssl-requirement">{{ t('pythonSanitycheck.manualInstall.requirements.ssl') }}</li>
            </ul>
          </div>
        </div>
      </n-spin>
    </n-card>
  </div>
</template>

<script>
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event";
import { NButton, NSpin } from 'naive-ui'
import { useI18n } from 'vue-i18n';
import { useAppStore } from '../../store'
import CheckResultsList from '../CheckResultsList.vue'

export default {
  name: 'PythonSanitycheck',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin, CheckResultsList },
  setup() {
    const { t } = useI18n()
    return { t }
  },
  data: () => ({
    os: undefined,
    loading: true,
    python_sane: false,
    checkResults: [],
    installing_python: false,
    appStore: useAppStore(),
    _unlistenPythonInstall: null,
  }),
  watch: {
    python_sane(newValue) {
      if (newValue && !this.loading && this.nextstep) {
        setTimeout(() => {
          this.nextstep();
        }, 250);
      }
    }
  },
  methods: {
    check_python_sanity: function () {
      this.loading = true;
      invoke("python_sanity_check", {}).then((results) => {
        this.checkResults = results || [];
        this.python_sane = this.checkResults.length > 0 && this.checkResults.every((r) => r.passed);
      }).catch((error) => {
        console.error("Python sanity check failed:", error);
        this.checkResults = [];
        this.python_sane = false;
      }).finally(() => {
        this.loading = false;
      });
    },
    get_os: function () {
      this.appStore.getOs().then((os) => {
        this.os = os.toLowerCase();
      }).catch((error) => {
        console.error("Failed to get OS:", error);
      });
    },
    install_python: async function () {
      this.installing_python = true;
      this.loading = true; // show the spinner over the whole card

      // Set up a one-shot listener BEFORE invoking the install command,
      // so we don't miss the completion event.
      try {
        // Clean up any prior listener just in case
        if (this._unlistenPythonInstall) {
          this._unlistenPythonInstall();
          this._unlistenPythonInstall = null;
        }

        // Promise that resolves when the backend emits python-install-complete,
        // or rejects on a 5-minute timeout.
        const installCompleted = new Promise(async (resolve, reject) => {
          const timeoutId = setTimeout(() => {
            if (this._unlistenPythonInstall) {
              this._unlistenPythonInstall();
              this._unlistenPythonInstall = null;
            }
            reject(new Error("Python installation timed out"));
          }, 5 * 60 * 1000);

          this._unlistenPythonInstall = await listen("python-install-complete", (event) => {
            clearTimeout(timeoutId);
            if (this._unlistenPythonInstall) {
              this._unlistenPythonInstall();
              this._unlistenPythonInstall = null;
            }
            resolve(event.payload);
          });
        });

        // Kick off the install (returns immediately on the Rust side)
        await invoke("python_install", {});

        // Wait for the real completion event
        const payload = await installCompleted;
        console.log("Python install complete:", payload);
      } catch (error) {
        console.error("Python installation failed:", error);
      } finally {
        this.installing_python = false;
        // Re-run the sanity check regardless — even on failure, the user
        // should see the current state instead of a stuck spinner.
        this.check_python_sanity();
      }
    },
  },
  mounted() {
    this.check_python_sanity();
    this.get_os();
  },
  beforeUnmount() {
    if (this._unlistenPythonInstall) {
      this._unlistenPythonInstall();
      this._unlistenPythonInstall = null;
    }
  }
}
</script>

<style scoped>
.python-check {
  padding: 2rem;
  max-width: 800px;
  margin: 0 auto;
}

.title {
  font-size: 1.8rem;
  color: #374151;
  margin-bottom: 2rem;
}

.status-card {
  background: white;
  min-height: 300px;
}

.status-content {
  padding: 1rem;
}

.action-buttons {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  align-items: center;
}

.install-section {
  text-align: center;
}

.install-note {
  color: #6b7280;
  font-size: 0.875rem;
  margin-top: 0.5rem;
}

.manual-instructions {
  margin-top: 2rem;
  padding: 1.5rem;
  background: #fee2e2;
  border-radius: 0.5rem;
}

.manual-instructions h3 {
  color: #991b1b;
  margin-bottom: 1rem;
}

.manual-instructions ul {
  list-style: disc;
  padding-left: 1.5rem;
  color: #374151;
}

.manual-instructions li {
  margin-bottom: 0.5rem;
}

.n-card {
  border: none;
}

.n-result .n-result-icon .n-base-icon {
  color: blue;
}

.n-button {
  background-color: #E8362D;
}
</style>

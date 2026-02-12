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
import { ref, watch } from "vue";
import { useI18n } from 'vue-i18n';
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin } from 'naive-ui'
import loading from "naive-ui/es/_internal/loading";
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
    appStore: useAppStore()
  }),
  watch: {
    python_sane(newValue) {
      // Auto-navigate when Python check passes
      if (newValue && !this.loading && this.nextstep) {
        setTimeout(() => {
          this.nextstep();
        }, 250);
      }
    }
  },
  methods: {
    check_python_sanity: async function () {
      this.loading = true;
      const results = await invoke("python_sanity_check", {});
      this.checkResults = results || [];
      this.python_sane = this.checkResults.length > 0 && this.checkResults.every((r) => r.passed);
      this.loading = false;
      return false;
    },
    get_os: async function () {
      this.os = await this.appStore.getOs();
      this.os = this.os.toLowerCase();
      return false;
    },
    install_python: async function () {
      this.installing_python = true;
      await invoke("python_install", {});
      this.installing_python = false;
      this.check_python_sanity();
      return false;
    },
  },
  mounted() {
    this.check_python_sanity();
    this.get_os();
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

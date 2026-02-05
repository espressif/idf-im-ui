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
              </div>
            </template>
          </n-result>

          <!-- Check Results List -->
          <div v-if="check_results.length > 0" class="check-results" data-id="check-results">
            <p v-if="failed_count > 0" class="check-summary" data-id="check-summary">
              {{ t('pythonSanitycheck.failed.summary') }}
            </p>
            <div v-for="result in check_results" :key="result.check_type" 
                 :class="['check-item', result.passed ? 'check-passed' : 'check-failed']"
                 :data-id="'check-' + result.check_type">
              <span class="check-status">{{ result.passed ? '✓' : '✗' }}</span>
              <span class="check-label">{{ result.label }}</span>
              <span v-if="!result.passed" class="check-message">
                {{ t('pythonSanitycheck.failed.message', { check: result.label }) }}
              </span>
            </div>
          </div>

          <div v-if="!python_sane && os !== 'windows'" class="recheck-section" data-id="recheck-section">
            <n-button @click="check_python_sanity" type="error" data-id="recheck-python-button">
              {{ t('pythonSanitycheck.actions.recheckInstallation') }}
            </n-button>
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

export default {
  name: 'PythonSanitycheck',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin },
  setup() {
    const { t } = useI18n()
    return { t }
  },
  data: () => ({
    os: undefined,
    loading: true,
    python_sane: false,
    check_results: [],
    installing_python: false,
    appStore: useAppStore()
  }),
  computed: {
    failed_count() {
      return this.check_results.filter(r => !r.passed).length;
    }
  },
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
      const response = await invoke("python_sanity_check", {});
      this.python_sane = response.all_passed;
      this.check_results = response.results || [];
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

.check-results {
  margin-top: 1.5rem;
  padding: 1rem;
  background: #f9fafb;
  border-radius: 0.5rem;
}

.check-summary {
  margin: 0 0 1rem 0;
  font-weight: 500;
  color: #374151;
}

.check-item {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  padding: 0.5rem 0;
  border-bottom: 1px solid #e5e7eb;
}

.check-item:last-child {
  border-bottom: none;
}

.check-status {
  width: 1.5rem;
  font-weight: bold;
  flex-shrink: 0;
}

.check-passed .check-status {
  color: #10b981;
}

.check-failed .check-status {
  color: #ef4444;
}

.check-label {
  flex: 1;
  font-weight: 500;
  color: #374151;
}

.check-message {
  width: 100%;
  margin-top: 0.25rem;
  margin-left: 1.5rem;
  font-size: 0.875rem;
  color: #6b7280;
}

.check-failed .check-message {
  color: #dc2626;
}

.recheck-section {
  margin-top: 2rem;
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

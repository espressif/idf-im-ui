<template>
  <div class="python-check" data-id="python-check">

    <n-card class="status-card" data-id="python-status-card">
      <n-spin :show="loading" data-id="python-check-spinner">
        <div v-if="!loading" class="status-content" data-id="python-status-content">
          <n-result :status="python_sane ? 'success' : 'warning'"
            :title="python_sane ? 'Python Environment Ready' : 'Python Setup Required'"
            :description="python_sane ? 'Your Python installation meets all requirements' : 'Python 3.10+ with pip, virtualenv, and SSL support is required'"
            data-id="python-check-result">
            <template #footer>
              <div class="action-buttons" data-id="python-action-buttons">
                <div v-if="!python_sane && os === 'windows'" class="install-section" data-id="python-install-section">
                  <n-button @click="install_python" type="warning" :loading="installing_python" :disabled="loading"
                    data-id="install-python-button">
                    {{ installing_python ? 'Installing Python...' : 'Install Python' }}
                  </n-button>
                  <p class="install-note" data-id="install-python-note">This will install Python with all required
                    components</p>
                </div>

                <n-button v-if="python_sane" @click="nextstep" type="error" :disabled="loading"
                  data-id="continue-button">
                  Continue to Next Step
                </n-button>
              </div>
            </template>
          </n-result>

          <div v-if="!python_sane && os !== 'windows'" class="manual-instructions"
            data-id="manual-install-instructions">
            <h3 data-id="manual-install-title">Manual Installation Required</h3>
            <p data-id="manual-install-intro">Please install:</p>
            <ul data-id="manual-install-requirements">
              <li data-id="python-requirement">Python 3.10 or later</li>
              <li data-id="pip-requirement">pip package manager</li>
              <li data-id="virtualenv-requirement">virtualenv module</li>
              <li data-id="ssl-requirement">SSL support</li>
            </ul>
            <n-button @click="check_python_sanity" type="error" data-id="recheck-python-button">
              Recheck Python Installation
            </n-button>
          </div>
        </div>
      </n-spin>
    </n-card>
  </div>
</template>

<script>
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin } from 'naive-ui'
import loading from "naive-ui/es/_internal/loading";

export default {
  name: 'PythonSanitycheck',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin },
  data: () => ({
    os: undefined,
    loading: true,
    python_sane: false,
    installing_python: false,
  }),
  methods: {
    check_python_sanity: async function () {
      this.loading = true;
      this.python_sane = await invoke("python_sanity_check", {});;
      this.loading = false;
      return false;
    },
    get_os: async function () {
      this.os = await invoke("get_operating_system", {});
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

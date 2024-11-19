<template>
  <div class="python-check">
    <h1 class="title">Python Environment Check</h1>

    <n-card class="status-card">
      <n-spin :show="loading">
        <div v-if="!loading" class="status-content">
          <n-result :status="python_sane ? 'success' : 'warning'"
            :title="python_sane ? 'Python Environment Ready' : 'Python Setup Required'"
            :description="python_sane ? 'Your Python installation meets all requirements' : 'Python 3.10+ with pip, virtualenv, and SSL support is required'">
            <template #footer>
              <div class="action-buttons">
                <div v-if="!python_sane && os === 'windows'" class="install-section">
                  <n-button @click="install_python" type="warning" :loading="installing_python" :disabled="loading">
                    {{ installing_python ? 'Installing Python...' : 'Install Python' }}
                  </n-button>
                  <p class="install-note">This will install Python with all required components</p>
                </div>

                <n-button v-if="python_sane" @click="nextstep" type="error" :disabled="loading">
                  Continue to Next Step
                </n-button>
              </div>
            </template>
          </n-result>

          <div v-if="!python_sane && os !== 'windows'" class="manual-instructions">
            <h3>Manual Installation Required</h3>
            <p>Please install:</p>
            <ul>
              <li>Python 3.10 or later</li>
              <li>pip package manager</li>
              <li>virtualenv module</li>
              <li>SSL support</li>
            </ul>
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
</style>
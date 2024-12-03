<template>
  <div class="prerequisites">
    <h1 class="title">Prerequisites Check</h1>
    <p class="description">The installer will now verify required components for ESP-IDF...</p>

    <div class="check-section">
      <n-card class="prerequisites-list">
        <template #header>
          <div class="card-header">
            <span class="header-title">Prerequisites</span>

          </div>
        </template>

        <n-spin :show="loading">
          <ul class="items-list">
            <li v-for="p in display_prerequisities" :key="p.name"
              :class="{ 'item': true, 'missing': p.icon === '❌', 'installed': p.icon === '✔' }">
              <span class="item-icon">{{ p.icon }}</span>
              <span class="item-name">{{ p.name }}</span>
            </li>
          </ul>
        </n-spin>

      </n-card>
      <n-button @click="check_prerequisites" type="error" :loading="loading">
        {{ loading ? 'Checking...' : 'Check Prerequisites' }}
      </n-button>

      <!-- Results Section -->
      <div v-if="did_the_check_run" class="results-section">
        <n-result v-if="missing_prerequisities.length === 0" status="success" title="All Prerequisites Installed"
          description="Your system is ready for ESP-IDF installation">
          <template #footer>
            <n-button @click="nextstep" type="error">
              Continue to Next Step
            </n-button>
          </template>
        </n-result>

        <n-result v-else status="warning" title="Missing Prerequisites"
          :description="os === 'windows' ? 'Click below to automatically install missing components' : 'Please install the following components manually'">
          <template #footer>
            <div class="missing-items">
              <div v-if="os === 'windows'" class="windows-install">
                <n-button @click="install_prerequisites" type="warning" :loading="installing_prerequisities">
                  Install Missing Prerequisites
                </n-button>
              </div>
              <div v-else class="manual-install">
                <p class="hint">Please install these components and run the check again:</p>
                <ul class="missing-list">
                  <li v-for="p in missing_prerequisities" :key="p">{{ p }}</li>
                </ul>
              </div>
            </div>
          </template>
        </n-result>
      </div>
    </div>
  </div>
</template>

<script>
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin } from 'naive-ui'
import loading from "naive-ui/es/_internal/loading";

export default {
  name: 'PrerequisitiesCheck',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin },
  data: () => ({
    loading: false,
    installing_prerequisities: false,
    did_the_check_run: false,
    all_prerequisities: [],
    missing_prerequisities: [],
    display_prerequisities: [],
    os: undefined,
  }),
  methods: {
    get_prerequisities_list: async function () {
      this.loading = true;
      this.all_prerequisities = await invoke("get_prequisites", {});;
      this.loading = false;
      this.display_prerequisities = this.all_prerequisities.map(p => ({
        name: p,
        icon: '❓',
      }));
      return false;
    },
    check_prerequisites: async function () {
      this.loading = true;
      const missing_list = await invoke("check_prequisites", {});
      this.missing_prerequisities = missing_list;
      console.log("missing prerequisities: ", missing_list);
      this.display_prerequisities = this.display_prerequisities.map(p => ({
        name: p.name,
        icon: missing_list.includes(p.name) ? '❌' : '✔',
      }));
      this.did_the_check_run = true;
      this.loading = false;
      return false;
    },
    install_prerequisites: async function () {
      this.installing_prerequisities = true;
      await invoke("install_prerequisites", {});
      this.check_prerequisites();
      this.installing_prerequisities = false;
      return false;
    },
    get_os: async function () {
      this.os = await invoke("get_operating_system", {});;
      return false;
    },
  },
  mounted() {
    this.get_prerequisities_list();
    this.get_os();
  }
}
</script>

<style scoped>
.prerequisites {
  padding: 2rem;
  max-width: 1200px;
  margin: 0 auto;
  align-items: center;
  justify-content: center;
  text-align: center;
}

.title {
  font-size: 1.8rem;
  color: #374151;
  margin-bottom: 0.5rem;
}

.description {
  color: #6b7280;
  margin-bottom: 2rem;
}

.check-section {
  display: flex;
  flex-direction: column;
  gap: 2rem;
}

.prerequisites-list {
  background: white;
}

.card-header {
  display: flex;
  justify-content: center;
  align-items: center;
}

.header-title {
  font-size: 1.2rem;
  font-weight: 500;
}

.items-list {
  list-style: none;
  padding: 0;
  margin: 0;
}

.item {
  display: flex;
  align-items: center;
  padding: 0.75rem;
  border-bottom: 1px solid #e5e7eb;
}

.item:last-child {
  border-bottom: none;
}

.item-icon {
  margin-right: 1rem;
  font-size: 1.2rem;
}

.item-name {
  color: #374151;
}

.missing {
  background-color: #fee2e2;
}

.installed {
  background-color: #ecfdf5;
}

.results-section {
  margin-top: 2rem;
}

.missing-items {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.missing-list {
  list-style: disc;
  padding-left: 1.5rem;
  color: #e7352c;
}

.hint {
  color: #6b7280;
  margin-bottom: 0.5rem;
}
</style>
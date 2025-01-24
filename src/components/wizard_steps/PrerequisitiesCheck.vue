<template>
  <div class="prerequisites" data-id="prerequisites-check">
    <h1 class="title" data-id="prerequisites-title">Prerequisites Check</h1>
    <p class="description" data-id="prerequisites-description">The installer will now verify required components for
      ESP-IDF...</p>

    <div class="check-section" data-id="check-section">
      <n-card class="prerequisites-list" data-id="prerequisites-card">
        <template #header>
          <div class="card-header" data-id="prerequisites-card-header">
            <span class="header-title" data-id="prerequisites-header-title">Prerequisites</span>
          </div>
        </template>

        <n-spin :show="loading" data-id="prerequisites-spinner">
          <ul class="items-list" data-id="prerequisites-items-list">
            <li v-for="p in display_prerequisities" :key="p.name"
              :class="{ 'item': true, 'missing': p.icon === '❌', 'installed': p.icon === '✔' }"
              :data-id="`prerequisite-item-${p.name}`">
              <span class="item-name" :data-id="`prerequisite-name-${p.name}`">{{ p.name }}</span>
              <span class="item-icon" :data-id="`prerequisite-icon-${p.name}`">{{ p.icon }}</span>
            </li>
          </ul>
          <n-progress type="line" color="#ff0000" rail-color="#ffgg00" :percentage="percentage" />
        </n-spin>
      </n-card>
      <div v-if="did_the_check_run">
        <div v-if="missing_prerequisities.length === 0">
          <n-button @click="nextstep" type="error" data-id="continue-button">
            Continue to Next Step
          </n-button>
        </div>

        <div v-else>
          <p>{{ os === 'windows' ? 'Click below to automatically install missing components' :
            'Please install the following components manually' }}</p>
          <div v-if="os === 'windows'" class="windows-install" data-id="windows-install-section">
            <n-button @click="install_prerequisites" type="warning" :loading="installing_prerequisities"
              data-id="install-prerequisites-button">
              Install Missing Prerequisites
            </n-button>
          </div>
          <div v-else class="manual-install" data-id="manual-install-section">
            <p class="hint" data-id="manual-install-hint">Please install these components and run the check again:
            </p>
          </div>
        </div>
      </div>
      <div v-else>
        <n-button @click="check_prerequisites" type="error" :loading="loading" data-id="check-prerequisites-button">
          {{ loading ? 'Checking...' : 'Check Prerequisites' }}
        </n-button>
      </div>

    </div>
  </div>
</template>

<script>
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin, NProgress } from 'naive-ui'

export default {
  name: 'PrerequisitiesCheck',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin, NProgress },
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
  computed: {
    percentage() {
      return Math.ceil(this.all_prerequisities.length === 0 ? 0 : ((this.all_prerequisities.length - this.missing_prerequisities.length) / this.all_prerequisities.length) * 100);
    }
  },
  mounted() {
    this.get_prerequisities_list();
    this.get_os();
    this.check_prerequisites();
  }
}
</script>

<style scoped>
.prerequisites {
  padding: 2rem;
  padding-top: 0px;
  max-width: 1200px;
  margin: 0 auto;
  align-items: center;
  justify-content: center;
  text-align: center;
}

.title {
  font-family: 'Trueno-bold';
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
  border: none;
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
  margin-bottom: 5px;
  align-items: center;
  padding: 0.75rem;
  border-bottom: 1px solid #e5e7eb;
  text-align: center;
}

.item:last-child {
  border-bottom: none;
}

.item-icon {
  margin-left: 1rem;
  font-size: 1.2rem;
  border: 1px solid #6da4d0;
  border-radius: 50%;
  width: 1.8rem;
  text-align: center;
  color: #6da4d0;
}

.item-name {
  flex-grow: 5;
  color: #374151;
}

.missing {
  background-color: #feeaea;
}

.installed {
  background-color: #fafafa;
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

.n-progress {
  width: 50%;
  margin: auto;
  margin-top: 2rem;
}
</style>
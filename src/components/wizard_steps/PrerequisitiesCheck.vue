<template>
  <div class="prerequisites" data-id="prerequisites-check">
    <h1 class="title" data-id="prerequisites-title">Prerequisites Check</h1>
    <p class="description" data-id="prerequisites-description">The installer will now verify required components for
      ESP-IDF...</p>

    <div class="check-section" data-id="check-section">
      <n-card class="prerequisites-list" data-id="prerequisites-card">

        <div class="loading-overlay-wrapper">
          <n-spin :show="loading" description="Checking prerequisites..." data-id="prerequisites-spinner">
            <ul class="items-list" :class="{ 'overlay-active': loading }" data-id="prerequisites-items-list">
              <li v-for="p in display_prerequisities" :key="p.name"
                :class="{ 'item': true, 'missing': p.icon === '❌', 'installed': p.icon === '✔' }"
                :data-id="`prerequisite-item-${p.name}`">
                <span class="item-name" :data-id="`prerequisite-name-${p.name}`">{{ p.name }}</span>
                <span class="item-icon" :data-id="`prerequisite-icon-${p.name}`">{{ p.icon }}</span>
              </li>
            </ul>
            <n-progress type="line" color="#ff0000" rail-color="#ffgg00" :percentage="percentage" />
          </n-spin>
          <div class="overlay" data-id="prerequisites-overlay" :style="loading ? { display: 'block' } : { display: 'none' }"></div>
        </div>
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
            <p class="hint" data-id="manual-install-hint">Please install the prerequisites and run the check again.
            </p>
          </div>
        </div>
      </div>
      <div v-if="missing_prerequisities.length > 0 || !did_the_check_run">
        <n-button @click="check_prerequisites" type="error" :loading="loading" data-id="check-prerequisites-button">
          {{ loading ? 'Checking...' : 'Check Prerequisites' }}
        </n-button>
      </div>

    </div>
  </div>
</template>

<script>
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin, NProgress, NCard } from 'naive-ui' // Added NCard here

export default {
  name: 'PrerequisitiesCheck',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin, NProgress, NCard }, // Added NCard here
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
      this.all_prerequisities = await invoke("get_prequisites", {});
      this.display_prerequisities = this.all_prerequisities.map(p => ({
        name: p,
        icon: '❓',
      }));
      return false;
    },
    check_prerequisites: async function () {
      this.loading = true;
      setTimeout(() => {
        invoke("check_prequisites", {}).then(missing_list => {
          this.missing_prerequisities = missing_list;
          console.log("missing prerequisities: ", missing_list);
          this.did_the_check_run = true;
          this.loading = false;
          this.display_prerequisities = this.display_prerequisities.map(p => ({
            name: p.name,
            icon: missing_list.includes(p.name) ? '❌' : '✔',
          }));
        });
      }, 400);

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
  position: relative; /* Added for positioning the overlay */
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
  position: relative; /* Added for positioning the overlay */
  z-index: 1; /* Ensure list content is below overlay */
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

/* Enhanced spinner styles */
:deep(.n-spin-container) {
  position: relative;
  padding: 2rem;
}

:deep(.n-spin) {
  transform: scale(1.5);
  position: absolute; /* Position the spinner over the overlay */
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  z-index: 3; /* Ensure spinner is above the overlay */
}

:deep(.n-spin .n-spin-body) {
  color: var(--espressif-red-color);
}

:deep(.n-spin-description) {
  font-size: 1.1rem;
  color: #374151;
  margin-top: 1rem;
}

/* New styles for the overlay */
.loading-overlay-wrapper {
  position: relative; /* Container for the list and overlay */
}

.overlay {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background-color: rgba(123, 80, 80, 0.7); /* Dark overlay with some transparency */
  z-index: 20; /* Ensure overlay is above the list items but below the spinner */
  border-radius: inherit; /* Inherit border-radius from n-card if any */
}

/* When overlay is active, make list items less interactive */
.items-list.overlay-active {
  pointer-events: none;
}
</style>

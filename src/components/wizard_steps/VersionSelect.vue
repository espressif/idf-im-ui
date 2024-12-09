<template>
  <div class="version-select">
    <h1 class="title">Select ESP-IDF Versions</h1>
    <p class="description">Choose which ESP-IDF SDK versions to install:</p>

    <n-card class="selection-card">
      <n-spin :show="loading">
        <div class="versions-grid">
          <div v-for="version in versions" :key="version.name" class="version-item"
            :class="{ 'selected': version.selected }">
            <n-checkbox v-model:checked="version.selected">
              <div class="version-content">
                <div class="version-header">
                  <span class="version-name">{{ version.name }}</span>
                  <n-tag v-if="version.latest" type="error" size="small">Latest</n-tag>
                  <n-tag v-if="version.lts" type="success" size="small">LTS</n-tag>
                </div>
                <span v-if="version.description" class="version-description">
                  {{ version.description }}
                </span>
              </div>
            </n-checkbox>
          </div>
        </div>

        <!-- <div class="summary-section" v-if="selectedVersions.length > 0">
          <div class="summary-content">
            <span class="summary-label">Selected versions:</span>
            <div class="selected-tags">
              <n-tag v-for="version in selectedVersions" :key="version" type="error" closable
                @close="deselectVersion(version)">
                {{ version }}
              </n-tag>
            </div>
          </div>
        </div> -->

        <div class="action-footer">
          <n-button @click="processVersions" type="error" size="large" :disabled="!hasSelectedVersions">
            Continue Installation
          </n-button>
        </div>
      </n-spin>
    </n-card>
  </div>
</template>

<script>
import { ref, version } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin, NCard, NCheckbox, NTag } from 'naive-ui'
import loading from "naive-ui/es/_internal/loading";

export default {
  name: 'VersionSelect',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin, NCard, NCheckbox, NTag },
  data: () => ({
    loading: true,
    versions: [],
  }),
  methods: {
    get_available_versions: async function () {
      const versions = await invoke("get_idf_versions", {});
      this.versions = versions;
      this.loading = false;
    },
    async processVersions() {
      await invoke("set_versions", { versions: this.selectedVersions });
      this.nextstep();
    },
    deselectVersion(versionName) {
      const version = this.versions.find(v => v.name === versionName);
      if (version) {
        version.selected = false;
      }
    }
  },
  computed: {
    hasSelectedVersions() {
      return this.versions.some(version => version.selected);
    },
    selectedVersions() {
      return this.versions
        .filter(version => version.selected)
        .map(version => version.name);
    }
  },
  mounted() {
    this.get_available_versions();
  }
}
</script>

<style scoped>
.version-select {
  padding: 2rem;
  max-width: 800px;
  margin: 0 auto;
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

.selection-card {
  background: white;
  padding: 1rem;
}

.versions-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 1rem;
  margin-bottom: 2rem;
}

.version-item {
  padding: 1rem;
  border: 1px solid #e5e7eb;
  border-radius: 0.5rem;
  transition: all 0.2s ease;
}

.version-item:hover {
  border-color: #e7352c;
}

.version-item.selected {
  background-color: #fee2e2;
  border-color: #e7352c;
}

.version-content {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.version-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.version-name {
  font-weight: 500;
  color: #374151;
}

.version-description {
  font-size: 0.875rem;
  color: #6b7280;
}

.version-meta {
  display: flex;
  justify-content: space-between;
  font-size: 0.75rem;
  color: #6b7280;
}

.summary-section {
  margin: 2rem 0;
  padding: 1rem;
  background-color: #f9fafb;
  border-radius: 0.5rem;
}

.summary-content {
  display: flex;
  align-items: center;
  gap: 1rem;
}

.summary-label {
  font-weight: 500;
  color: #374151;
}

.selected-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
}

.action-footer {
  display: flex;
  justify-content: center;
  margin-top: 2rem;
  padding-top: 1rem;
  border-top: 1px solid #e5e7eb;
}
</style>

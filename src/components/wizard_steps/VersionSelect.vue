<template>
  <div class="version-select" data-id="version-select">
    <h1 class="title" data-id="version-select-title">{{ t('versionSelect.title') }}</h1>
    <p class="description" data-id="version-select-description">{{ t('versionSelect.description') }}</p>

    <n-card class="selection-card" data-id="version-selection-card">
      <n-spin :show="loading" data-id="version-loading-spinner">
        <div class="versions-grid" data-id="versions-grid">
          <div v-for="version in versions" :key="version.name" class="version-item"
            :class="{ 'selected': selected_versions.includes(version.name) }" :data-id="`version-item-${version.name}`"
            @click="clickOnVersion">
            <div class="version-content" :data-id="`version-content-${version.name}`">
              <div class="version-header" :data-id="`version-header-${version.name}`">
                <span class="version-name" :data-id="`version-name-${version.name}`">{{ version.name }}</span>
                <n-tag v-if="version.latest" type="error" size="small"
                  :data-id="`version-latest-tag-${version.name}`">{{ t('versionSelect.tags.latest') }}</n-tag>
                <n-tag v-if="version.lts" type="success" size="small"
                  :data-id="`version-lts-tag-${version.name}`">{{ t('versionSelect.tags.lts') }}</n-tag>
              </div>
              <span v-if="version.description" class="version-description"
                :data-id="`version-description-${version.name}`">
                {{ version.description }}
              </span>
            </div>
          </div>
        </div>
        <div class="selected-versions-summary" v-if="selected_versions.length > 0">
          {{ t('versionSelect.selectedVersions') }}
          <div class="selected-tags" data-id="selected-tags">
            <n-tag v-for="version in selected_versions" :key="version" closable round size="medium"
              :data-id="`selected-tag-${version}`" @close="deselectVersion(version)">
              {{ version }}
            </n-tag>
          </div>
        </div>

        <div class="action-footer" data-id="version-action-footer">
          <n-button @click="processVersions" type="error" size="large" :disabled="!hasSelectedVersions"
            data-id="continue-installation-button">
            {{ t('versionSelect.continueInstallation') }}
          </n-button>
        </div>
      </n-spin>
    </n-card>
  </div>
</template>

<script>
import { ref, version } from "vue";
import { useI18n } from 'vue-i18n';
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin, NCard, NCheckbox, NTag } from 'naive-ui'
import loading from "naive-ui/es/_internal/loading";

export default {
  name: 'VersionSelect',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin, NCard, NCheckbox, NTag },
  setup() {
    const { t } = useI18n()
    return { t }
  },
  data: () => ({
    loading: true,
    versions: [],
    selected_versions: []
  }),
  methods: {
    get_available_versions: async function () {
      const versions = await invoke("get_idf_versions", {includeUnstable: true});
      this.versions = versions;
      this.loading = false;
    },
    async processVersions() {
      await invoke("set_versions", { versions: this.selectedVersions });
      this.nextstep();
    },
    deselectVersion(versionName) {
      this.selected_versions.splice(this.selected_versions.indexOf(versionName), 1);
    },
    clickOnVersion(event) {
      let version_name = event.currentTarget.textContent.trim();
      console.log('target clicked', version_name);
      if (this.selected_versions.includes(version_name)) {
        this.selected_versions.splice(this.selected_versions.indexOf(version_name), 1);
      } else {
        this.selected_versions.push(version_name);
      }
      console.log('selected versions', this.selected_versions);
    }
  },
  computed: {
    hasSelectedVersions() {
      return this.selected_versions.length > 0;
    },
    selectedVersions() {
      return this.selected_versions
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
  font-size: 27px;
  font-family: 'Trueno-bold', sans-serif;
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
  display: flex;
  /* Use flexbox */
  flex-wrap: wrap;
  /* Allow items to wrap to the next line */
  gap: 12px;
  margin-bottom: 2rem;
}

.version-item {
  width: 125px;
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0.5rem;
  border: 1px solid #e5e7eb;
  border-radius: 0.5rem;
  cursor: pointer;
  transition: all 0.2s ease;
}

.version-item:hover {
  border-color: #1290d8;
}

.version-item.selected {
  background-color: #1290d8;
  border-color: #1290d8;
  color: white
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

.version-item.selected .version-name {
  color: white;
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
}

.n-card {
  border: none;
  border-top: 1px solid #e5e7eb;

}

.selected-versions-summary {
  font-family: 'Trueno-light';
  display: flex;
  font-size: 21px;
  line-height: 23px;
  vertical-align: baseline;
}

.selected-tags {
  margin-left: 1rem;
}

.n-tag {
  color: #1290d8;
  border-color: #1290d8;
  border-radius: 3px;

}
</style>


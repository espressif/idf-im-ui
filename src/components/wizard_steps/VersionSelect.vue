<template>
  <div class="version-select" data-id="version-select">
    <h1 class="title" data-id="version-select-title">{{ t('versionSelect.title') }}</h1>
    <p class="description" data-id="version-select-description">{{ t('versionSelect.description') }}</p>

    <n-card class="selection-card" data-id="version-selection-card">
      <n-spin :show="loading" data-id="version-loading-spinner">
        <!-- Stable Versions Section -->
        <version-section
          v-if="stableVersions.length > 0"
          :title="t('versionSelect.sections.stable.title')"
          :description="t('versionSelect.sections.stable.description')"
          :versions="stableVersions"
          :selected="selected_versions"
          @toggle="toggleVersion"
        />

        <!-- Pre-release Versions Section -->
        <version-section
          v-if="preReleaseVersions.length > 0"
          :title="t('versionSelect.sections.preRelease.title')"
          :description="t('versionSelect.sections.preRelease.description')"
          :versions="preReleaseVersions"
          :selected="selected_versions"
          @toggle="toggleVersion"
          type="prerelease"
        />

        <!-- Development (Master) Section -->
        <version-section
          v-if="masterVersion"
          :title="t('versionSelect.sections.development.title')"
          :description="t('versionSelect.sections.development.description')"
          :versions="[masterVersion]"
          :selected="selected_versions"
          @toggle="toggleVersion"
          type="development"
        />

        <!-- Selected Versions Summary -->
        <div class="selected-versions-summary" v-if="selected_versions.length > 0" data-id="selected-summary">
          <span class="summary-label">{{ t('versionSelect.selectedVersions') }}</span>
          <div class="selected-tags" data-id="selected-tags">
            <n-tag
              v-for="version in selected_versions"
              :key="version"
              closable
              round
              size="medium"
              :type="getTagType(version)"
              :data-id="`selected-tag-${version}`"
              @close="deselectVersion(version)"
            >
              {{ version }}
            </n-tag>
          </div>
        </div>

        <!-- Action Footer -->
        <div class="action-footer" data-id="version-action-footer">
          <n-button
            @click="processVersions"
            type="primary"
            size="large"
            :disabled="!hasSelectedVersions"
            data-id="continue-installation-button"
          >
            {{ t('versionSelect.continueInstallation') }}
          </n-button>
        </div>
      </n-spin>
    </n-card>
  </div>
</template>

<script>
import { ref } from "vue";
import { useI18n } from 'vue-i18n';
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin, NCard, NCheckbox, NTag, NAlert } from 'naive-ui';
import VersionSection from './VersionSection.vue';

export default {
  name: 'VersionSelect',
  props: {
    nextstep: Function
  },
  components: {
    NButton, NSpin, NCard, NCheckbox, NTag, NAlert,
    VersionSection,
  },
  setup() {
    const { t } = useI18n();
    return { t };
  },
  data: () => ({
    loading: true,
    versions: [],
    selected_versions: []
  }),
  computed: {
    stableVersions() {
      return this.versions.filter(v => v.category === 'stable' && !v.is_master);
    },
    preReleaseVersions() {
      return this.versions.filter(v => v.category === 'pre_release' && !v.is_master);
    },
    masterVersion() {
      return this.versions.find(v => v.is_master);
    },
    hasSelectedVersions() {
      return this.selected_versions.length > 0;
    },
  },
  methods: {
    async getAvailableVersions() {
      try {
        const versions = await invoke("get_idf_versions", { includeUnstable: true });
        this.versions = versions;
        this.selected_versions = versions.filter(v => v.selected).map(v => v.name);
      } catch (error) {
        console.error('Error fetching versions:', error);
      } finally {
        this.loading = false;
      }
    },
    async processVersions() {
      await invoke("set_versions", { versions: this.selected_versions });
      this.nextstep();
    },
    toggleVersion(versionName) {
      const index = this.selected_versions.indexOf(versionName);
      if (index > -1) {
        this.selected_versions.splice(index, 1);
      } else {
        this.selected_versions.push(versionName);
      }
    },
    deselectVersion(versionName) {
      const index = this.selected_versions.indexOf(versionName);
      if (index > -1) {
        this.selected_versions.splice(index, 1);
      }
    },
    getTagType(versionName) {
      if (versionName === 'master') return 'error';
      const version = this.versions.find(v => v.name === versionName);
      if (version?.pre_release) return 'warning';
      return 'success';
    }
  },
  mounted() {
    this.getAvailableVersions();
  }
};
</script>

<style scoped>
.version-select {
  padding: 24px;
  max-width: 900px;
  margin: 0 auto;
}

.title {
  font-size: 28px;
  font-weight: 600;
  margin-bottom: 8px;
  color: var(--n-text-color);
}

.description {
  color: var(--n-text-color-3);
  margin-bottom: 24px;
  font-size: 14px;
}

.selection-card {
  border-radius: 12px;
}

.selected-versions-summary {
  margin-top: 24px;
  padding: 16px;
  background-color: var(--n-color-embedded);
  border-radius: 10px;
}

.summary-label {
  display: block;
  font-weight: 500;
  margin-bottom: 12px;
  color: var(--n-text-color);
}

.selected-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.action-footer {
  margin-top: 24px;
  display: flex;
  justify-content: flex-end;
}
</style>

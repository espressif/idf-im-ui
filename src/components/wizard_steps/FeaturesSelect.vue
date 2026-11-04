<template>
  <div class="features-select" data-id="features-select">
    <h1 class="title" data-id="features-select-title">{{ t('featuresSelect.title') }}</h1>
    <p class="description" data-id="features-select-description">{{ t('featuresSelect.description') }}</p>

    <n-card class="features-card" data-id="features-card">
      <n-spin :show="loading" data-id="features-loading-spinner">
        <template v-if="!loading && versionFeatures.length > 0">
          <!-- Version Tabs -->
          <n-tabs
            v-if="versionFeatures.length > 1"
            v-model:value="activeVersion"
            type="line"
            class="version-tabs"
            data-id="version-tabs"
          >
            <n-tab-pane
              v-for="versionData in versionFeatures"
              :key="versionData.version"
              :name="versionData.version"
              :tab="versionData.version"
              :data-id="`version-tab-${versionData.version}`"
              :tab-props="{ 'data-id': `version-tab-button-${versionData.version}` }"
            >
              <features-content
                :version="versionData.version"
                :required-features="getRequiredFeatures(versionData.version)"
                :optional-features="getOptionalFeatures(versionData.version)"
                :selected-features="selectedFeaturesMap[versionData.version] || []"
                tabbed
                @toggle-feature="toggleFeature"
                @select-all="selectAllOptional"
                @deselect-all="deselectAllOptional"
              />
            </n-tab-pane>
          </n-tabs>

          <!-- Single version (no tabs needed) -->
          <features-content
            v-else
            :version="versionFeatures[0].version"
            :required-features="getRequiredFeatures(versionFeatures[0].version)"
            :optional-features="getOptionalFeatures(versionFeatures[0].version)"
            :selected-features="selectedFeaturesMap[versionFeatures[0].version] || []"
            @toggle-feature="toggleFeature"
            @select-all="selectAllOptional"
            @deselect-all="deselectAllOptional"
          />

          <div class="action-footer" data-id="features-action-footer">
            <span class="selection-summary" data-id="selection-summary">
              {{ t('featuresSelect.summaryMultiVersion', {
                versions: versionFeatures.length,
                details: selectionSummary
              }) }}
            </span>
            <n-button
              @click="processChoices"
              type="info"
              size="large"
              :disabled="!canProceed"
              data-id="continue-features-button"
            >
              {{ t('featuresSelect.continueButton') }}
            </n-button>
          </div>
        </template>

        <template v-else-if="!loading && versionFeatures.length === 0">
          <div class="empty-state" data-id="empty-state">
            <p class="empty-message">{{ t('featuresSelect.noFeatures') }}</p>
          </div>
        </template>
      </n-spin>
    </n-card>
  </div>
</template>

<script>
import { ref, computed } from "vue";
import { useI18n } from 'vue-i18n';
import { invoke } from "@tauri-apps/api/core";
import { NButton, NSpin, NCard, NTabs, NTabPane } from 'naive-ui'
import FeaturesContent from './FeaturesContent.vue';

export default {
  name: 'FeaturesSelect',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin, NCard, NTabs, NTabPane, FeaturesContent },
  setup() {
    const { t } = useI18n()
    return { t }
  },
  data: () => ({
    loading: true,
    // Array of { version: string, features: FeatureInfo[] }
    versionFeatures: [],
    // Map of version -> selected feature names
    selectedFeaturesMap: {},
    // Currently active tab
    activeVersion: null,
  }),
  computed: {
    selectionSummary() {
      return this.versionFeatures.map(vf => {
        const selected = this.selectedFeaturesMap[vf.version]?.length || 0;
        const total = vf.features.length;
        return `${vf.version}: ${selected}/${total}`;
      }).join(', ');
    },
    canProceed() {
      // Check that each version has at least the required features selected
      return this.versionFeatures.every(vf => {
        const required = this.getRequiredFeatures(vf.version);
        const selected = this.selectedFeaturesMap[vf.version] || [];
        return required.every(rf => selected.includes(rf.name));
      });
    }
  },
  methods: {
    async getAvailableFeatures() {
      try {
        this.loading = true;

        // Fetch features for all versions
        const versionFeatures = await invoke("get_features_list_all_versions", {});
        this.versionFeatures = versionFeatures;

        // Initialize selected features map with required features for each version
        const initialMap = {};
        for (const vf of versionFeatures) {
          const required = vf.features.filter(f => !f.optional).map(f => f.name);
          initialMap[vf.version] = [...required];
        }
        this.selectedFeaturesMap = initialMap;

        // Set active tab to first version
        if (versionFeatures.length > 0) {
          this.activeVersion = versionFeatures[0].version;
        }

        // Try to restore previously saved selections
        try {
          const savedFeatures = await invoke("get_selected_features_per_version", {});
          if (savedFeatures && Object.keys(savedFeatures).length > 0) {
            // Merge saved features with required features
            for (const [version, features] of Object.entries(savedFeatures)) {
              if (this.selectedFeaturesMap[version]) {
                const required = this.getRequiredFeatures(version).map(f => f.name);
                // Ensure required features are always included
                const merged = [...new Set([...required, ...features])];
                this.selectedFeaturesMap[version] = merged;
              }
            }
          }
        } catch (err) {
          console.log("No previously saved features per version");
        }

        this.loading = false;
      } catch (error) {
        console.error("Failed to load features:", error);
        this.loading = false;
      }
    },

    getFeaturesForVersion(version) {
      const vf = this.versionFeatures.find(v => v.version === version);
      return vf ? vf.features : [];
    },

    getRequiredFeatures(version) {
      return this.getFeaturesForVersion(version).filter(f => !f.optional);
    },

    getOptionalFeatures(version) {
      return this.getFeaturesForVersion(version).filter(f => f.optional);
    },

    toggleFeature(version, featureName) {
      const feature = this.getFeaturesForVersion(version).find(f => f.name === featureName);

      // Don't allow toggling required features
      if (feature && !feature.optional) {
        return;
      }

      const selected = this.selectedFeaturesMap[version] || [];
      const index = selected.indexOf(featureName);

      if (index > -1) {
        selected.splice(index, 1);
      } else {
        selected.push(featureName);
      }

      // Trigger reactivity
      this.selectedFeaturesMap = { ...this.selectedFeaturesMap, [version]: selected };
    },

    selectAllOptional(version) {
      const allFeatureNames = this.getFeaturesForVersion(version).map(f => f.name);
      this.selectedFeaturesMap = { ...this.selectedFeaturesMap, [version]: [...allFeatureNames] };
    },

    deselectAllOptional(version) {
      const required = this.getRequiredFeatures(version).map(f => f.name);
      this.selectedFeaturesMap = { ...this.selectedFeaturesMap, [version]: [...required] };
    },

    async processChoices() {
      console.log("Selected features per version:", this.selectedFeaturesMap);

      if (!this.loading) {
        try {
          await invoke("set_selected_features_per_version", {
            featuresMap: this.selectedFeaturesMap
          });
          this.nextstep();
        } catch (error) {
          console.error("Failed to save features:", error);
        }
      }
    }
  },

  mounted() {
    this.getAvailableFeatures();
  }
}
</script>

<style scoped src="../styles/wizard-step-header.css"></style>

<style scoped>
.features-select {
  padding: 2rem;
  max-width: 1000px;
  margin: 0 auto;
}

.features-card {
  background: white;
  padding: 1.5rem;
}

.version-tabs {
  margin-bottom: 1rem;
}

.n-card__content {
  padding: 0px;
}

.n-button {
  padding: 5px;
}
</style>

<style scoped src="../styles/selection-step.css"></style>

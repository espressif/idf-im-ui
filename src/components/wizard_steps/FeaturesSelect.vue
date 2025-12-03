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
            >
              <div class="features-content" data-id="features-content">
                <div class="features-sections" data-id="features-sections">

                  <!-- Required Features Section -->
                  <div class="feature-section" data-id="required-section">
                    <div class="section-header">
                      <h3 class="section-title" data-id="required-title">
                        {{ t('featuresSelect.sections.required') }}
                      </h3>
                      <span class="feature-count">{{ getRequiredFeatures(versionData.version).length }}</span>
                    </div>
                    <div class="feature-group" data-id="required-group">
                      <div
                        v-for="feature in getRequiredFeatures(versionData.version)"
                        :key="`${versionData.version}-${feature.name}`"
                        class="feature-row required"
                        :data-id="`feature-row-${versionData.version}-${feature.name}`"
                      >
                        <div class="feature-checkbox-wrapper">
                          <n-checkbox
                            :checked="true"
                            disabled
                            :data-id="`feature-checkbox-${versionData.version}-${feature.name}`"
                          />
                        </div>
                        <div class="feature-info">
                          <span class="feature-name" :data-id="`feature-name-${versionData.version}-${feature.name}`">
                            {{ feature.name }}
                          </span>
                          <span
                            v-if="feature.description"
                            class="feature-desc"
                            :data-id="`feature-desc-${versionData.version}-${feature.name}`"
                          >
                            {{ feature.description }}
                          </span>
                        </div>
                      </div>
                    </div>
                  </div>

                  <!-- Optional Features Section -->
                  <div class="feature-section" data-id="optional-section">
                    <div class="section-header">
                      <h3 class="section-title" data-id="optional-title">
                        {{ t('featuresSelect.sections.optional') }}
                      </h3>
                      <div class="section-actions">
                        <n-button
                          @click="selectAllOptional(versionData.version)"
                          size="small"
                          text
                          type="primary"
                          data-id="select-all-button"
                        >
                          {{ t('featuresSelect.actions.selectAll') }}
                        </n-button>
                        <span class="divider">|</span>
                        <n-button
                          @click="deselectAllOptional(versionData.version)"
                          size="small"
                          text
                          type="primary"
                          data-id="deselect-all-button"
                        >
                          {{ t('featuresSelect.actions.deselectAll') }}
                        </n-button>
                      </div>
                    </div>
                    <div class="feature-group" data-id="optional-group">
                      <div
                        v-for="feature in getOptionalFeatures(versionData.version)"
                        :key="`${versionData.version}-${feature.name}`"
                        class="feature-row optional"
                        :class="{ 'selected': isFeatureSelected(versionData.version, feature.name) }"
                        :data-id="`feature-row-${versionData.version}-${feature.name}`"
                        @click="toggleFeature(versionData.version, feature.name)"
                      >
                        <div class="feature-checkbox-wrapper">
                          <n-checkbox
                            :checked="isFeatureSelected(versionData.version, feature.name)"
                            :data-id="`feature-checkbox-${versionData.version}-${feature.name}`"
                            @update:checked="() => toggleFeature(versionData.version, feature.name)"
                          />
                        </div>
                        <div class="feature-info">
                          <span class="feature-name" :data-id="`feature-name-${versionData.version}-${feature.name}`">
                            {{ feature.name }}
                          </span>
                          <span
                            v-if="feature.description"
                            class="feature-desc"
                            :data-id="`feature-desc-${versionData.version}-${feature.name}`"
                          >
                            {{ feature.description }}
                          </span>
                        </div>
                      </div>
                    </div>
                  </div>

                </div>
              </div>
            </n-tab-pane>
          </n-tabs>

          <!-- Single version (no tabs needed) -->
          <template v-else>
            <div class="features-content" data-id="features-content">
              <div class="features-sections" data-id="features-sections">

                <!-- Required Features Section -->
                <div class="feature-section" data-id="required-section">
                  <div class="section-header">
                    <h3 class="section-title" data-id="required-title">
                      {{ t('featuresSelect.sections.required') }}
                    </h3>
                    <span class="feature-count">{{ getRequiredFeatures(versionFeatures[0].version).length }}</span>
                  </div>
                  <div class="feature-group" data-id="required-group">
                    <div
                      v-for="feature in getRequiredFeatures(versionFeatures[0].version)"
                      :key="feature.name"
                      class="feature-row required"
                      :data-id="`feature-row-${feature.name}`"
                    >
                      <div class="feature-checkbox-wrapper">
                        <n-checkbox
                          :checked="true"
                          disabled
                          :data-id="`feature-checkbox-${feature.name}`"
                        />
                      </div>
                      <div class="feature-info">
                        <span class="feature-name" :data-id="`feature-name-${feature.name}`">
                          {{ feature.name }}
                        </span>
                        <span
                          v-if="feature.description"
                          class="feature-desc"
                          :data-id="`feature-desc-${feature.name}`"
                        >
                          {{ feature.description }}
                        </span>
                      </div>
                    </div>
                  </div>
                </div>

                <!-- Optional Features Section -->
                <div class="feature-section" data-id="optional-section">
                  <div class="section-header">
                    <h3 class="section-title" data-id="optional-title">
                      {{ t('featuresSelect.sections.optional') }}
                    </h3>
                    <div class="section-actions">
                      <n-button
                        @click="selectAllOptional(versionFeatures[0].version)"
                        size="small"
                        text
                        type="primary"
                        data-id="select-all-button"
                      >
                        {{ t('featuresSelect.actions.selectAll') }}
                      </n-button>
                      <span class="divider">|</span>
                      <n-button
                        @click="deselectAllOptional(versionFeatures[0].version)"
                        size="small"
                        text
                        type="primary"
                        data-id="deselect-all-button"
                      >
                        {{ t('featuresSelect.actions.deselectAll') }}
                      </n-button>
                    </div>
                  </div>
                  <div class="feature-group" data-id="optional-group">
                    <div
                      v-for="feature in getOptionalFeatures(versionFeatures[0].version)"
                      :key="feature.name"
                      class="feature-row optional"
                      :class="{ 'selected': isFeatureSelected(versionFeatures[0].version, feature.name) }"
                      :data-id="`feature-row-${feature.name}`"
                      @click="toggleFeature(versionFeatures[0].version, feature.name)"
                    >
                      <div class="feature-checkbox-wrapper">
                        <n-checkbox
                          :checked="isFeatureSelected(versionFeatures[0].version, feature.name)"
                          :data-id="`feature-checkbox-${feature.name}`"
                          @update:checked="() => toggleFeature(versionFeatures[0].version, feature.name)"
                        />
                      </div>
                      <div class="feature-info">
                        <span class="feature-name" :data-id="`feature-name-${feature.name}`">
                          {{ feature.name }}
                        </span>
                        <span
                          v-if="feature.description"
                          class="feature-desc"
                          :data-id="`feature-desc-${feature.name}`"
                        >
                          {{ feature.description }}
                        </span>
                      </div>
                    </div>
                  </div>
                </div>

              </div>
            </div>
          </template>

          <div class="action-footer" data-id="features-action-footer">
            <span class="selection-summary" data-id="selection-summary">
              {{ t('featuresSelect.summaryMultiVersion', {
                versions: versionFeatures.length,
                details: selectionSummary
              }) }}
            </span>
            <n-button
              @click="processChoices"
              type="error"
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
import { NButton, NSpin, NCard, NCheckbox, NTabs, NTabPane } from 'naive-ui'

export default {
  name: 'FeaturesSelect',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin, NCard, NCheckbox, NTabs, NTabPane },
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

    isFeatureSelected(version, featureName) {
      const selected = this.selectedFeaturesMap[version] || [];
      return selected.includes(featureName);
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

<style scoped>
.features-select {
  padding: 2rem;
  max-width: 1000px;
  margin: 0 auto;
}

.title {
  font-size: 27px;
  font-family: 'Trueno-bold', sans-serif;
  color: #374151;
  margin-bottom: 0.5rem;
}

.description {
  font-size: 21px;
  font-family: 'Trueno-light', sans-serif;
  color: #6b7280;
  margin-bottom: 2rem;
}

.features-card {
  background: white;
  padding: 1.5rem;
}

.version-tabs {
  margin-bottom: 1rem;
}

.features-content {
  margin-bottom: 1.5rem;
}

.features-sections {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.feature-section {
  display: flex;
  flex-direction: column;
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 0.75rem;
  padding-bottom: 0.5rem;
  border-bottom: 1px solid #e5e7eb;
}

.section-title {
  font-size: 1rem;
  font-weight: 600;
  color: #374151;
  margin: 0;
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.feature-count {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 1.5rem;
  height: 1.5rem;
  padding: 0 0.375rem;
  background-color: #e5e7eb;
  color: #6b7280;
  border-radius: 0.75rem;
  font-size: 0.75rem;
  font-weight: 500;
}

.section-actions {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.875rem;
}

.divider {
  color: #d1d5db;
}

.feature-group {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.feature-row {
  display: flex;
  align-items: flex-start;
  gap: 0.75rem;
  padding: 0.625rem 0.75rem;
  border: 1px solid #e5e7eb;
  border-radius: 0.375rem;
  background: white;
  transition: all 0.2s ease;
}

.feature-row.optional {
  cursor: pointer;
}

.feature-row.optional:hover {
  border-color: #e7352c;
  background-color: #fef2f2;
}

.feature-row.optional.selected {
  background-color: #fee2e2;
  border-color: #e7352c;
}

.feature-row.required {
  background-color: #f0f9ff;
  border-color: #bfdbfe;
  cursor: default;
}

.feature-checkbox-wrapper {
  display: flex;
  align-items: center;
  padding-top: 0.125rem;
}

.feature-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  pointer-events: none;
}

.feature-name {
  font-size: 0.875rem;
  font-weight: 500;
  color: #374151;
}

.feature-desc {
  font-size: 0.8125rem;
  color: #6b7280;
  line-height: 1.4;
}

.action-footer {
  display: flex;
  justify-content: center;
  align-items: center;
  gap: 1rem;
  margin-top: 2rem;
  padding-top: 1rem;
}

.selection-summary {
  font-size: 0.875rem;
  color: #6b7280;
}

.empty-state {
  padding: 3rem;
  text-align: center;
}

.empty-message {
  font-size: 1rem;
  color: #6b7280;
}

.n-card {
  border: none;
  border-top: 1px solid #e5e7eb;
  padding: 0px;
}

.n-card__content {
  padding: 0px;
}

.n-button[type="primary"] {
  background-color: #E8362D;
  color: #e5e7eb;
}

.n-button {
  padding: 5px;
  background-color: #E8362D;
  color: #ffffff;
}
</style>

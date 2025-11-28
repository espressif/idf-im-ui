<template>
  <div class="features-select" data-id="features-select">
    <h1 class="title" data-id="features-select-title">{{ t('featuresSelect.title') }}</h1>
    <p class="description" data-id="features-select-description">{{ t('featuresSelect.description') }}</p>

    <n-card class="features-card" data-id="features-card">
      <n-spin :show="loading" data-id="features-loading-spinner">
        <template v-if="!loading && features.length > 0">
          <div class="features-content" data-id="features-content">
            <!-- Single column with grouped sections -->
            <div class="features-sections" data-id="features-sections">

              <!-- Required Features Section -->
              <div class="feature-section" data-id="required-section">
                <div class="section-header">
                  <h3 class="section-title" data-id="required-title">
                    {{ t('featuresSelect.sections.required') }}
                  </h3>
                  <span class="feature-count">{{ requiredFeatures.length }}</span>
                </div>
                <div class="feature-group" data-id="required-group">
                  <div
                    v-for="feature in requiredFeatures"
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
                      @click="selectAllOptional"
                      size="small"
                      text
                      type="primary"
                      data-id="select-all-button"
                    >
                      {{ t('featuresSelect.actions.selectAll') }}
                    </n-button>
                    <span class="divider">|</span>
                    <n-button
                      @click="deselectAllOptional"
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
                    v-for="feature in optionalFeatures"
                    :key="feature.name"
                    class="feature-row optional"
                    :class="{ 'selected': isFeatureSelected(feature.name) }"
                    :data-id="`feature-row-${feature.name}`"
                    @click="toggleFeature(feature.name)"
                  >
                    <div class="feature-checkbox-wrapper">
                      <n-checkbox
                        :checked="isFeatureSelected(feature.name)"
                        :data-id="`feature-checkbox-${feature.name}`"
                        @update:checked="(value) => toggleFeature(feature.name)"
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

          <div class="action-footer" data-id="features-action-footer">
            <span class="selection-summary" data-id="selection-summary">
              {{ t('featuresSelect.summary', {
                selected: selectedFeatures.length,
                total: features.length
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

        <template v-else-if="!loading && features.length === 0">
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
import { NButton, NSpin, NCard, NCheckbox } from 'naive-ui'

export default {
  name: 'FeaturesSelect',
  props: {
    nextstep: Function
  },
  components: { NButton, NSpin, NCard, NCheckbox },
  setup() {
    const { t } = useI18n()
    return { t }
  },
  data: () => ({
    loading: true,
    features: [],
    selectedFeatures: [],
  }),
  computed: {
    requiredFeatures() {
      return this.features.filter(f => !f.optional);
    },
    optionalFeatures() {
      return this.features.filter(f => f.optional);
    },
    canProceed() {
      return this.selectedFeatures.length >= this.requiredFeatures.length;
    }
  },
  methods: {
    async getAvailableFeatures() {
      try {
        this.loading = true;
        const features = await invoke("get_features_list", {});
        this.features = features;

        this.selectedFeatures = this.requiredFeatures.map(f => f.name);

        try {
          const savedFeatures = await invoke("get_selected_features", {});
          if (savedFeatures && savedFeatures.length > 0) {
            this.selectedFeatures = savedFeatures;
          }
        } catch (err) {
          console.log("No previously saved features");
        }

        this.loading = false;
      } catch (error) {
        console.error("Failed to load features:", error);
        this.loading = false;
      }
    },

    isFeatureSelected(featureName) {
      return this.selectedFeatures.includes(featureName);
    },

    toggleFeature(featureName) {
      const feature = this.features.find(f => f.name === featureName);

      if (feature && !feature.optional) {
        return;
      }

      const index = this.selectedFeatures.indexOf(featureName);
      if (index > -1) {
        this.selectedFeatures.splice(index, 1);
      } else {
        this.selectedFeatures.push(featureName);
      }
    },

    selectAllOptional() {
      const allFeatureNames = this.features.map(f => f.name);
      this.selectedFeatures = [...allFeatureNames];
    },

    deselectAllOptional() {
      this.selectedFeatures = this.requiredFeatures.map(f => f.name);
    },

    async processChoices() {
      console.log("Selected features:", this.selectedFeatures);

      if (!this.loading) {
        try {
          await invoke("set_selected_features", {
            features: this.selectedFeatures
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

<template>
  <div class="features-content" data-id="features-content">
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
          <feature-row
            v-for="feature in requiredFeatures"
            :key="feature.name"
            :feature="feature"
            :version="version"
            :is-required="true"
            :tabbed="tabbed"
          />
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
              @click="$emit('select-all', version)"
              size="small"
              text
              type="info"
              data-id="select-all-button"
            >
              {{ t('featuresSelect.actions.selectAll') }}
            </n-button>
            <span class="divider">|</span>
            <n-button
              @click="$emit('deselect-all', version)"
              size="small"
              text
              type="info"
              data-id="deselect-all-button"
            >
              {{ t('featuresSelect.actions.deselectAll') }}
            </n-button>
          </div>
        </div>
        <div class="feature-group" data-id="optional-group">
          <feature-row
            v-for="feature in optionalFeatures"
            :key="feature.name"
            :feature="feature"
            :version="version"
            :is-selected="selectedFeatures.includes(feature.name)"
            :tabbed="tabbed"
            @toggle="(v, f) => $emit('toggle-feature', v, f)"
          />
        </div>
      </div>

    </div>
  </div>
</template>

<script>
import { useI18n } from 'vue-i18n';
import { NButton } from 'naive-ui';
import FeatureRow from './FeatureRow.vue';

export default {
  name: 'FeaturesContent',
  components: { NButton, FeatureRow },
  props: {
    version: {
      type: String,
      required: true
    },
    requiredFeatures: {
      type: Array,
      required: true
    },
    optionalFeatures: {
      type: Array,
      required: true
    },
    selectedFeatures: {
      type: Array,
      required: true
    },
    tabbed: {
      type: Boolean,
      default: false
    }
  },
  emits: ['toggle-feature', 'select-all', 'deselect-all'],
  setup() {
    const { t } = useI18n()
    return { t }
  }
}
</script>

<style scoped>
.features-content {
  margin-bottom: 1.5rem;
}

.features-sections {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.feature-group {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.n-button {
  padding: 5px;
}
</style>

<style scoped src="../styles/option-section.css"></style>

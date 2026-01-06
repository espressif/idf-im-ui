<template>
  <div class="version-section" :data-id="`${type}-versions-section`">
    <div class="section-header" :data-id="`${type}-section-header`">
      <h2 class="section-title">{{ title }}</h2>
    </div>
    <p v-if="description && type != 'prerelease' && type != 'development'" class="section-description" :data-id="`${type}-section-description`">
      {{ description }}
    </p>
    <n-alert
      v-if="type === 'prerelease' || type === 'development'"
      :type="type === 'development' ? 'warning' : 'info'"
      :show-icon="true"
      class="section-alert"
      :class="{ warning: type === 'development' }"
      :data-id="`${type}-alert`"
    >
      <template v-if="type === 'development'" #header>
        {{ t('versionSelect.sections.development.warningTitle') }}
      </template>
      {{ description }}
    </n-alert>
    <div class="versions-grid" :data-id="`${type}-versions-grid`">
      <version-item
        v-for="version in versions"
        :key="version.name"
        :version="version"
        :selected="selected.includes(version.name)"
        :type="type"
        @toggle="$emit('toggle', $event)"
      />
    </div>
  </div>
</template>

<script>
import { NAlert } from 'naive-ui';
import { useI18n } from 'vue-i18n';
import VersionItem from './VersionItem.vue';

export default {
  name: 'VersionSection',
  components: { NAlert, VersionItem },
  props: {
    title: String,
    description: String,
    versions: Array,
    selected: Array,
    type: String,
  },
  setup() {
    const { t } = useI18n();
    return { t };
  },
};
</script>

<style scoped>
.version-section {
  margin-bottom: 32px;
}

.version-section:last-of-type {
  margin-bottom: 24px;
}

.section-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 8px;
}

.section-title {
  font-size: 18px;
  font-weight: 600;
  margin: 0;
  color: var(--n-text-color);
}

.section-description {
  color: var(--n-text-color-3);
  font-size: 13px;
  margin-bottom: 16px;
  padding-left: 30px;
}

.section-alert {
  margin-bottom: 16px;
}

.section-alert.warning {
  border-left: 4px solid #f0a020;
}

.versions-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: 12px;
}
</style>

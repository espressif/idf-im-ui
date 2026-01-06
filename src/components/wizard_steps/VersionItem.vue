<template>
  <div
    class="version-item"
    :class="[{ 'selected': selected }, type]"
    :data-id="`version-item-${version.name}`"
    @click="toggle"
  >
    <div class="version-content" :data-id="`version-content-${version.name}`">
      <div class="version-header" :data-id="`version-header-${version.name}`">
        <span class="version-name" :data-id="`version-name-${version.name}`">{{ version.name }}</span>
        <div class="version-tags">
          <n-tag
            v-if="version.latest"
            type="error"
            size="small"
            :data-id="`version-latest-tag-${version.name}`"
          >
            {{ t('versionSelect.tags.latest') }}
          </n-tag>
          <n-tag
            v-if="version.pre_release"
            type="warning"
            size="small"
            :data-id="`version-prerelease-tag-${version.name}`"
          >
            {{ t('versionSelect.tags.preRelease') }}
          </n-tag>
          <n-tag
            v-if="version.is_master"
            type="error"
            size="small"
            :data-id="`version-unstable-tag-${version.name}`"
          >
            {{ t('versionSelect.tags.unstable') }}
          </n-tag>
        </div>
      </div>
    </div>
  </div>
</template>

<script>
import { NTag } from 'naive-ui';
import { useI18n } from 'vue-i18n';

export default {
  name: 'VersionItem',
  components: { NTag },
  props: {
    version: Object,
    selected: Boolean,
    type: String,
  },
  setup() {
    const { t } = useI18n();
    return { t };
  },
  methods: {
    toggle() {
      this.$emit('toggle', this.version.name);
    },
  },
};
</script>

<style scoped>
.version-item {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-direction: column;
  padding: 16px;
  border: 2px solid var(--n-border-color);
  border-radius: 10px;
  cursor: pointer;
  transition: all 0.2s ease;
  background-color: var(--n-card-color);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.version-item:hover {
  border-color: var(--n-primary-color);
  background-color: var(--n-primary-color-hover);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
}

.version-item.selected {
  border-color: var(--n-primary-color);
  background-color: var(--n-primary-color-suppl);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
}

.version-item.prerelease {
  border-style: dashed;
}

.version-item.development {
  border-color: var(--espressif-red-color);
  border-style: dashed;
}

.version-item.development:hover {
  border-color: var(--espressif-red-color);
  background-color: rgba(208, 48, 80, 0.05);
}

.version-item.development.selected {
  background-color: rgba(208, 48, 80, 0.1);
}

.version-content {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex: 1;
  text-align: center;
}

.version-header {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
}

.version-name {
  font-weight: 600;
  font-size: 15px;
  color: var(--n-text-color);
}

.version-tags {
  display: flex;
  gap: 4px;
}

.version-description {
  font-size: 12px;
  color: var(--n-text-color-3);
}

.development-warning {
  color: var(--espressif-red-color);
  font-style: italic;
}
</style>

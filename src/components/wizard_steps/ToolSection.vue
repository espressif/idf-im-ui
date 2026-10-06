<template>
  <div class="tool-section">
    <div class="section-header">
      <h3 class="section-title" :data-id="isRequired ? 'required-title' : 'optional-title'">
        {{ title }}
      </h3>
      <span v-if="isRequired" class="tool-count">{{ tools.length }}</span>
      <div v-else class="section-actions">
        <n-button
          @click="$emit('select-all', version)"
          text
          type="info"
          data-id="select-all-button"
        >
          {{ t('toolsSelect.actions.selectAll') }}
        </n-button>
        <span class="divider">|</span>
        <n-button
          @click="$emit('deselect-all', version)"
          text
          type="info"
          data-id="deselect-all-button"
        >
          {{ t('toolsSelect.actions.deselectAll') }}
        </n-button>
      </div>
    </div>
    <div class="tool-group" :data-id="isRequired ? 'required-group' : 'optional-group'">
      <tool-row
        v-for="tool in tools"
        :key="tool.name"
        :tool="tool"
        :version="version"
        :is-required="isRequired"
        :is-selected="isToolSelected(tool.name)"
        @toggle="(v, t) => $emit('toggle-tool', v, t)"
      />
      <div v-if="!isRequired && tools.length === 0" class="no-optional-tools">
        {{ t('toolsSelect.noOptionalTools') }}
      </div>
    </div>
  </div>
</template>

<script>
import { useI18n } from 'vue-i18n';
import { NButton } from 'naive-ui';
import ToolRow from './ToolRow.vue';

export default {
  name: 'ToolSection',
  components: { NButton, ToolRow },
  props: {
    title: {
      type: String,
      required: true
    },
    tools: {
      type: Array,
      required: true
    },
    selectedTools: {
      type: Array,
      required: true
    },
    version: {
      type: String,
      required: true
    },
    isRequired: {
      type: Boolean,
      default: false
    }
  },
  emits: ['toggle-tool', 'select-all', 'deselect-all'],
  setup() {
    const { t } = useI18n()
    return { t }
  },
  methods: {
    isToolSelected(toolName) {
      return this.selectedTools.includes(toolName);
    }
  }
}
</script>

<style scoped>
.tool-group {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  max-height: 300px;
  overflow-y: auto;
}

.no-optional-tools {
  padding: 1rem;
  text-align: center;
  color: #6b7280;
  font-style: italic;
}
.n-button {
  padding: 10px;
  height: auto;
  color:white;
}
</style>

<style scoped src="../styles/option-section.css"></style>

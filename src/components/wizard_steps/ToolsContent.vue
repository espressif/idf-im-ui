<template>
  <div class="tools-content" data-id="tools-content">
    <div class="tools-sections" data-id="tools-sections">

      <!-- Required Tools Section -->
      <tool-section
        :title="t('toolsSelect.sections.required')"
        :tools="requiredTools"
        :selected-tools="selectedTools"
        :version="version"
        :is-required="true"
        data-id="required-section"
      />

      <!-- Optional Tools Section -->
      <tool-section
        :title="t('toolsSelect.sections.optional')"
        :tools="optionalTools"
        :selected-tools="selectedTools"
        :version="version"
        :is-required="false"
        @toggle-tool="(v, t) => $emit('toggle-tool', v, t)"
        @select-all="(v) => $emit('select-all', v)"
        @deselect-all="(v) => $emit('deselect-all', v)"
        data-id="optional-section"
      />

    </div>
  </div>
</template>

<script>
import { useI18n } from 'vue-i18n';
import ToolSection from './ToolSection.vue';

export default {
  name: 'ToolsContent',
  components: { ToolSection },
  props: {
    version: {
      type: String,
      required: true
    },
    requiredTools: {
      type: Array,
      required: true
    },
    optionalTools: {
      type: Array,
      required: true
    },
    selectedTools: {
      type: Array,
      required: true
    }
  },
  emits: ['toggle-tool', 'select-all', 'deselect-all'],
  setup() {
    const { t } = useI18n()
    return { t }
  }
}
</script>

<style scoped>
.tools-content {
  margin-bottom: 1.5rem;
}

.tools-sections {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}
</style>

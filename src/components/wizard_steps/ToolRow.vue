<template>
  <div
    :class="rowClass"
    :data-id="version ? `tool-row-${version}-${tool.name}` : `tool-row-${tool.name}`"
    @click="handleClick"
  >
    <div class="tool-checkbox-wrapper">
      <n-checkbox
        :checked="isRequired || isSelected"
        :disabled="isRequired"
        :data-id="version ? `tool-checkbox-${version}-${tool.name}` : `tool-checkbox-${tool.name}`"
      />
    </div>
    <div class="tool-info">
      <span class="tool-name" :data-id="version ? `tool-name-${version}-${tool.name}` : `tool-name-${tool.name}`">
        {{ tool.name }}
      </span>
      <span
        v-if="tool.description"
        class="tool-desc"
        :data-id="version ? `tool-desc-${version}-${tool.name}` : `tool-desc-${tool.name}`"
      >
        {{ tool.description }}
      </span>
    </div>
  </div>
</template>

<script>
import { NCheckbox } from 'naive-ui';

export default {
  name: 'ToolRow',
  components: { NCheckbox },
  props: {
    tool: {
      type: Object,
      required: true
    },
    version: {
      type: String,
      required: true
    },
    isRequired: {
      type: Boolean,
      default: false
    },
    isSelected: {
      type: Boolean,
      default: false
    }
  },
  emits: ['toggle'],
  computed: {
    rowClass() {
      return {
        'tool-row': true,
        'required': this.isRequired,
        'optional': !this.isRequired,
        'selected': this.isSelected && !this.isRequired
      };
    }
  },
  methods: {
    handleClick() {
      if (!this.isRequired) {
        this.$emit('toggle', this.version, this.tool.name);
      }
    }
  }
}
</script>

<style scoped src="../styles/option-row.css"></style>

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

<style scoped>
.tool-row {
  display: flex;
  align-items: flex-start;
  gap: 0.75rem;
  padding: 0.625rem 0.75rem;
  border: 1px solid #e5e7eb;
  border-radius: 0.375rem;
  background: white;
  transition: all 0.2s ease;
}

.tool-row.optional {
  cursor: pointer;
}

.tool-row.optional:hover {
  border-color: #e7352c;
  background-color: #fef2f2;
}

.tool-row.optional.selected {
  background-color: #fee2e2;
  border-color: #e7352c;
}

.tool-row.required {
  background-color: #f0f9ff;
  border-color: #bfdbfe;
  cursor: default;
}

.tool-checkbox-wrapper {
  display: flex;
  align-items: center;
  padding-top: 0.125rem;
}

.tool-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  pointer-events: none;
}

.tool-name {
  font-size: 0.875rem;
  font-weight: 500;
  color: #374151;
}

.tool-desc {
  font-size: 0.8125rem;
  color: #6b7280;
  line-height: 1.4;
}
</style>

<template>
  <div
    class="feature-row"
    :class="isRequired ? 'required' : ['optional', { 'selected': isSelected }]"
    :data-id="`feature-row-${idSuffix}`"
    @click="handleClick"
  >
    <div class="feature-checkbox-wrapper">
      <n-checkbox
        :checked="isRequired || isSelected"
        :disabled="isRequired"
        :data-id="`feature-checkbox-${idSuffix}`"
        @click="handleCheckboxToggle"
        @update:checked="handleCheckboxToggle"
      />
    </div>
    <div class="feature-info">
      <span class="feature-name" :data-id="`feature-name-${idSuffix}`">
        {{ feature.name }}
      </span>
      <span
        v-if="feature.description"
        class="feature-desc"
        :data-id="`feature-desc-${idSuffix}`"
      >
        {{ feature.description }}
      </span>
    </div>
  </div>
</template>

<script>
import { NCheckbox } from 'naive-ui';

export default {
  name: 'FeatureRow',
  components: { NCheckbox },
  props: {
    feature: {
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
    },
    // Inside a version tab the data-ids carry the version, and the
    // checkbox itself also toggles (on click and on update:checked).
    tabbed: {
      type: Boolean,
      default: false
    }
  },
  emits: ['toggle'],
  computed: {
    idSuffix() {
      return this.tabbed ? `${this.version}-${this.feature.name}` : this.feature.name;
    }
  },
  methods: {
    handleClick() {
      if (!this.isRequired) {
        this.$emit('toggle', this.version, this.feature.name);
      }
    },
    handleCheckboxToggle() {
      if (this.tabbed && !this.isRequired) {
        this.$emit('toggle', this.version, this.feature.name);
      }
    }
  }
}
</script>

<style scoped src="../styles/option-row.css"></style>

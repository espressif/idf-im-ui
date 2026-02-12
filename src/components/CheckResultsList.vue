<template>
  <div v-if="items.length" class="check-results-list">
    <div
      v-for="(item, index) in items"
      :key="index"
      class="check-results-row"
      :class="{ 'check-results-row--failed': !item.passed }"
    >
      <span class="check-results-icon">{{ item.passed ? '✓' : '✗' }}</span>
      <div class="check-results-text">
        <span class="check-results-label">{{ item.display_name || item.displayName }}</span>
        <p v-if="item.hint" class="check-results-hint">{{ item.hint }}</p>
      </div>
    </div>
  </div>
</template>

<script>
export default {
  name: 'CheckResultsList',
  props: {
    /**
     * Array of check result items to display.
     * Each item should have: { passed: boolean, display_name?: string, displayName?: string, hint?: string }
     */
    items: {
      type: Array,
      required: true,
      default: () => []
    }
  }
}
</script>

<style scoped>
.check-results-list {
  width: 100%;
  max-width: 560px;
  margin: 1rem auto;
  text-align: left;
  border: 1px solid #e5e7eb;
  border-radius: 0.5rem;
  overflow: hidden;
}

.check-results-row {
  display: flex;
  align-items: flex-start;
  gap: 0.75rem;
  padding: 0.75rem 1rem;
  border-bottom: 1px solid #f3f4f6;
  background: #fafafa;
}

.check-results-row:last-child {
  border-bottom: none;
}

.check-results-row--failed {
  background: #fef2f2;
}

.check-results-icon {
  flex-shrink: 0;
  font-size: 1.125rem;
  font-weight: bold;
}

.check-results-row:not(.check-results-row--failed) .check-results-icon {
  color: #16a34a;
}

.check-results-row--failed .check-results-icon {
  color: #dc2626;
}

.check-results-text {
  flex: 1;
  min-width: 0;
}

.check-results-label {
  font-weight: 500;
  color: #374151;
}

.check-results-hint {
  margin: 0.25rem 0 0;
  font-size: 0.875rem;
  color: #6b7280;
  line-height: 1.4;
}
</style>

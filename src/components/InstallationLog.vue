<template>
  <n-collapse arrow-placement="right">
    <n-collapse-item :title="title" name="1">
      <template #header-extra>
        <span class="log-count">{{ countLabel }}</span>
      </template>

      <div class="log-container">
        <!-- Virtual scrolling container -->
        <div
          class="log-virtual-container"
          ref="container"
          @scroll="$emit('scroll', $event)"
        >
          <!-- Spacer for items above viewport -->
          <div
            class="virtual-spacer-top"
            :style="{ height: topSpacerHeight + 'px' }"
          ></div>

          <!-- Only render visible items -->
          <div class="log-scroll-container">
            <div
              v-for="(message, index) in logs"
              :key="`log-${startIndex + index}-${message.timestamp}`"
              class="log-entry"
              :style="{ height: itemHeight + 'px' }"
            >
              <pre
                class="log-message"
                :class="getLogMessageClass(message)"
                v-text="message.text"
              ></pre>
            </div>
          </div>

          <!-- Spacer for items below viewport -->
          <div
            class="virtual-spacer-bottom"
            :style="{ height: bottomSpacerHeight + 'px' }"
          ></div>
        </div>
      </div>
    </n-collapse-item>
  </n-collapse>
</template>

<script>
import { NCollapse, NCollapseItem } from 'naive-ui'

export default {
  name: 'InstallationLog',
  components: { NCollapse, NCollapseItem },
  props: {
    title: {
      type: String,
      required: true
    },
    countLabel: {
      type: String,
      required: true
    },
    logs: {
      type: Array,
      required: true
    },
    startIndex: {
      type: Number,
      required: true
    },
    itemHeight: {
      type: Number,
      required: true
    },
    topSpacerHeight: {
      type: Number,
      required: true
    },
    bottomSpacerHeight: {
      type: Number,
      required: true
    }
  },
  emits: ['scroll'],
  methods: {
    getLogMessageClass(message) {
      if (message.level === 'error') return 'log-message log-error';
      if (message.level === 'warning') return 'log-message log-warning';
      if (message.level === 'success') return 'log-message log-success';
      if (message.text && (message.text.includes('WARN') || message.text.includes('ERR'))) {
        return 'log-message highlight';
      }
      return 'log-message';
    }
  }
}
</script>

<style scoped>
.log-container {
  text-align: left;
  background-color: white;
}

.log-virtual-container {
  height: 300px;
  overflow-y: auto;
  overflow-x: hidden;
  will-change: scroll-position;
  -webkit-overflow-scrolling: touch;
  scroll-behavior: smooth;
}

.virtual-spacer-top,
.virtual-spacer-bottom {
  width: 100%;
  pointer-events: none;
}

.log-scroll-container {
  contain: layout style;
}

.log-entry {
  height: 24px;
  display: flex;
  align-items: flex-start;
  contain: layout;
  box-sizing: border-box;
}

.log-message {
  margin: 0;
  padding: 2px 4px;
  font-family: monospace;
  font-size: 0.85rem;
  line-height: 20px;
  text-rendering: optimizeSpeed;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  width: 100%;
  flex: 1;
}

/* Log level styling */
.log-message.log-error {
  background-color: #fee2e2;
  color: #b91c1c;
  border-left: 3px solid #ef4444;
}

.log-message.log-warning {
  background-color: #fef3c7;
  color: #d97706;
  border-left: 3px solid #f59e0b;
}

.log-message.log-success {
  color: #059669;
  border-left: 3px solid #10b981;
}

.log-message.highlight {
  background-color: #fff9c2;
  font-weight: 500;
  border-left: 3px solid var(--espressif-red-color);
}

.log-count {
  font-size: 0.8rem;
  color: #6b7280;
  font-weight: normal;
}

/* Scrollbar styling */
.log-virtual-container::-webkit-scrollbar {
  width: 8px;
}

.log-virtual-container::-webkit-scrollbar-track {
  background: #f1f1f1;
  border-radius: 4px;
}

.log-virtual-container::-webkit-scrollbar-thumb {
  background: #c1c1c1;
  border-radius: 4px;
}

.log-virtual-container::-webkit-scrollbar-thumb:hover {
  background: #a1a1a1;
}

/* Performance optimizations */
.log-virtual-container * {
  backface-visibility: hidden;
}

/* Responsive adjustments */
@media (max-width: 768px) {
  .log-virtual-container {
    height: 250px;
  }

  .log-entry {
    height: 28px;
  }

  .log-message {
    line-height: 24px;
  }
}
</style>

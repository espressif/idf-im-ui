<template>
  <div class="warning-banners" v-if="warnings.length > 0">
    <transition-group name="slide-down">
      <div
        v-for="warning in warnings"
        :key="warning.id"
        :class="['warning-banner', `warning-banner--${warning.type}`]"
        data-id="warning-banner"
      >
        <div class="warning-content">
          <div class="warning-icon">
            <!-- Warning icon (triangle with exclamation) -->
            <svg v-if="warning.type === 'warning'" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
              <path d="M1 21h22L12 2 1 21zm12-3h-2v-2h2v2zm0-4h-2v-4h2v4z"/>
            </svg>
            <!-- Info icon -->
            <svg v-else-if="warning.type === 'info'" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
              <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-6h2v6zm0-8h-2V7h2v2z"/>
            </svg>
            <!-- Error icon -->
            <svg v-else xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
              <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z"/>
            </svg>
          </div>
          <div class="warning-text">
            <span class="warning-title">{{ warning.title }}</span>
            <span v-if="warning.message" class="warning-message">{{ warning.message }}</span>
          </div>
          <button 
            v-if="warning.dismissible" 
            class="warning-close" 
            @click="dismissWarning(warning.id)" 
            data-id="dismiss-warning"
            :aria-label="$t('common.dismiss') || 'Dismiss'"
          >
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
              <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
            </svg>
          </button>
        </div>
      </div>
    </transition-group>
  </div>
</template>

<script>
import { computed } from 'vue'
import { useAppStore } from '../store'

export default {
  name: 'WarningBanner',
  setup() {
    const appStore = useAppStore()

    const warnings = computed(() => appStore.warnings)

    const dismissWarning = (id) => {
      appStore.removeWarning(id)
    }

    return {
      warnings,
      dismissWarning
    }
  }
}
</script>

<style scoped>
/* Warning Banner Base */
.warning-banner {
  width: 100%;
  padding: 0.75rem 1.5rem;
  z-index: 100;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

/* Warning type (amber/yellow) */
.warning-banner--warning {
  background: linear-gradient(135deg, #f59e0b 0%, #d97706 100%);
  color: white;
}

/* Info type (blue) */
.warning-banner--info {
  background: linear-gradient(135deg, #3b82f6 0%, #2563eb 100%);
  color: white;
}

/* Error type (red) */
.warning-banner--error {
  background: linear-gradient(135deg, #ef4444 0%, #dc2626 100%);
  color: white;
}

.warning-content {
  max-width: 1400px;
  margin: 0 auto;
  display: flex;
  align-items: center;
  gap: 1rem;
}

.warning-icon {
  flex-shrink: 0;
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.warning-icon svg {
  width: 24px;
  height: 24px;
}

.warning-text {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 0.125rem;
}

.warning-title {
  font-weight: 600;
  font-size: 0.9rem;
}

.warning-message {
  font-size: 0.85rem;
  opacity: 0.95;
}

.warning-close {
  flex-shrink: 0;
  background: transparent;
  border: none;
  color: white;
  width: 28px;
  height: 28px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background 0.2s ease;
  padding: 0;
}

.warning-close:hover {
  background: rgba(255, 255, 255, 0.2);
}

.warning-close svg {
  width: 18px;
  height: 18px;
}

/* Slide down animation */
.slide-down-enter-active,
.slide-down-leave-active {
  transition: all 0.3s ease;
}

.slide-down-enter-from {
  transform: translateY(-100%);
  opacity: 0;
}

.slide-down-leave-to {
  transform: translateY(-100%);
  opacity: 0;
}

/* Responsive Design */
@media (max-width: 768px) {
  .warning-banner {
    padding: 0.75rem 1rem;
  }

  .warning-content {
    gap: 0.75rem;
  }

  .warning-text {
    flex-direction: column;
  }

  .warning-title {
    font-size: 0.85rem;
  }

  .warning-message {
    font-size: 0.8rem;
  }
}
</style>

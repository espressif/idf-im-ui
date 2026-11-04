<template>
  <div class="current-activity" data-id="current-activity">
    <div class="current-step">
      <h3>{{ title }}</h3>
      <div class="activity-status">{{ activity }}</div>
      <div v-if="detail" class="activity-detail">{{ detail }}</div>
      <slot />
    </div>

    <div class="progress-section">
      <div class="progress-label">{{ progressLabel }}</div>
      <n-progress
        type="line"
        :percentage="progress"
        :processing="processing"
        :indicator-placement="'inside'"
        color="var(--espressif-red-color)"
      />
    </div>

    <!-- Installation Steps -->
    <div class="installation-steps" v-if="steps.length > 0">
      <div class="steps-container">
        <div
          v-for="(step, index) in steps"
          :key="index"
          class="step-item"
          :class="{
            'active': index === currentStep,
            'completed': index < currentStep,
            'pending': index > currentStep
          }"
        >
          <div class="step-indicator">{{ index + 1 }}</div>
          <div class="step-content">
            <div class="step-title">{{ step.title }}</div>
            <div class="step-description">{{ step.description }}</div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script>
import { NProgress } from 'naive-ui'

export default {
  name: 'InstallationActivity',
  components: { NProgress },
  props: {
    title: {
      type: String,
      required: true
    },
    activity: {
      type: String,
      default: ''
    },
    detail: {
      type: String,
      default: ''
    },
    progressLabel: {
      type: String,
      required: true
    },
    progress: {
      type: Number,
      default: 0
    },
    processing: {
      type: Boolean,
      default: false
    },
    steps: {
      type: Array,
      required: true
    },
    currentStep: {
      type: Number,
      required: true
    }
  }
}
</script>

<style scoped>
.current-activity {
  margin: 1rem 0;
  padding: 1rem;
  background-color: #f9fafb;
  border-radius: 8px;
  border-left: 4px solid #428ED2;
}

.current-step h3 {
  margin: 0 0 0.5rem 0;
  font-size: 1rem;
  color: #6b7280;
}

.activity-status {
  font-size: 1.1rem;
  font-weight: 500;
  color: #374151;
}

.activity-detail {
  font-size: 0.9rem;
  color: #6b7280;
  margin-top: 0.5rem;
}

.progress-section {
  margin-top: 1rem;
}

.progress-label {
  font-size: 0.875rem;
  color: #6b7280;
  margin-bottom: 0.5rem;
}

.installation-steps {
  margin-top: 1.5rem;
}

.steps-container {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 1rem;
}

.step-item {
  display: flex;
  align-items: center;
  padding: 0.75rem;
  border-radius: 8px;
  border: 1px solid #e5e7eb;
  transition: all 0.2s ease;
}

.step-item.active {
  border-color: #428ED2;
  background-color: #eff6ff;
}

.step-item.completed {
  border-color: #10b981;
  background-color: #f0fdf4;
}

.step-item.completed .step-indicator {
  background-color: #10b981;
  color: white;
}

.step-item.active .step-indicator {
  background-color: #428ED2;
  color: white;
}

.step-indicator {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background-color: #e5e7eb;
  color: #6b7280;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.75rem;
  font-weight: bold;
  margin-right: 0.75rem;
}

.step-content {
  flex: 1;
}

.step-title {
  font-weight: 500;
  color: #374151;
  font-size: 0.9rem;
}

.step-description {
  font-size: 0.8rem;
  color: #6b7280;
  margin-top: 0.25rem;
}
</style>

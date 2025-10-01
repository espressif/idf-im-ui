<template>
  <div class="global-progress" :class="progressClass">
    <div class="progress-content">
      <div class="progress-spinner" v-if="showSpinner">
        <n-spin :size="spinnerSize" />
      </div>

      <div class="progress-info" :class="{ 'has-spinner': showSpinner }">
        <div class="progress-message" v-if="currentMessage">
          {{ currentMessage }}
        </div>

        <n-progress
          v-if="showProgress"
          type="line"
          :percentage="progressPercentage"
          :indicator-placement="indicatorPlacement"
          :processing="processing"
          :color="progressColor"
          :rail-color="railColor"
          :height="progressHeight"
        />

        <div class="progress-details" v-if="showDetails">
          <span class="progress-status">{{ statusText }}</span>
          <span class="progress-time" v-if="estimatedTime">
            {{ $t('progress.estimatedTime', { time: estimatedTime }) }}
          </span>
        </div>
      </div>
    </div>

    <div class="progress-steps" v-if="steps.length > 0">
      <n-steps :current="currentStep" size="small">
        <n-step
          v-for="(step, index) in steps"
          :key="index"
          :title="step.title"
          :description="step.description"
          :status="getStepStatus(index)"
        />
      </n-steps>
    </div>
  </div>
</template>

<script>
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { NSpin, NProgress, NSteps, NStep } from 'naive-ui'
import { listen } from '@tauri-apps/api/event'

export default {
  name: 'GlobalProgress',
  components: {
    NSpin, NProgress, NSteps, NStep
  },
  props: {
    // Visual props
    messagePosition: {
      type: String,
      default: 'center', // left, center, right
      validator: (value) => ['left', 'center', 'right'].includes(value)
    },
    showSpinner: {
      type: Boolean,
      default: true
    },
    spinnerSize: {
      type: String,
      default: 'medium', // small, medium, large
      validator: (value) => ['small', 'medium', 'large'].includes(value)
    },
    showProgress: {
      type: Boolean,
      default: true
    },
    showDetails: {
      type: Boolean,
      default: false
    },
    progressHeight: {
      type: Number,
      default: 8
    },
    indicatorPlacement: {
      type: String,
      default: 'inside' // inside, outside, none
    },

    // Data props
    initialMessage: {
      type: String,
      default: 'progress.defaultMessage'
    },
    initialProgress: {
      type: Number,
      default: 0
    },
    steps: {
      type: Array,
      default: () => []
    },
    eventChannel: {
      type: String,
      default: 'progress-message'
    },

    // Color theme
    colorScheme: {
      type: String,
      default: 'primary', // primary, success, warning, error, info
      validator: (value) => ['primary', 'success', 'warning', 'error', 'info'].includes(value)
    }
  },
  setup(props) {
    const { t } = useI18n()
    const currentMessage = ref(props.initialMessage)
    const progressPercentage = ref(props.initialProgress)
    const processing = ref(true)
    const currentStep = ref(0)
    const statusText = ref(t('progress.status.inProgress'))
    const estimatedTime = ref('')
    const startTime = ref(Date.now())

    let unlisten = null
    let progressInterval = null

    const progressClass = computed(() => ({
      [`align-${props.messagePosition}`]: true,
      [`theme-${props.colorScheme}`]: true
    }))

    const progressColor = computed(() => {
      const colors = {
        primary: '#E8362D',
        success: '#52c41a',
        warning: '#faad14',
        error: '#ff4d4f',
        info: '#1290d8'
      }
      return colors[props.colorScheme] || colors.primary
    })

    const railColor = computed(() => {
      return props.colorScheme === 'error' ? '#ffccc7' : '#f0f0f0'
    })

    const getStepStatus = (index) => {
      if (index < currentStep.value) return 'finish'
      if (index === currentStep.value) return 'process'
      return 'wait'
    }

    const updateEstimatedTime = () => {
      if (progressPercentage.value > 0 && progressPercentage.value < 100) {
        const elapsed = Date.now() - startTime.value
        const rate = progressPercentage.value / elapsed
        const remaining = (100 - progressPercentage.value) / rate

        const minutes = Math.floor(remaining / 60000)
        const seconds = Math.floor((remaining % 60000) / 1000)

        if (minutes > 0) {
          estimatedTime.value = `${minutes}m ${seconds}s`
        } else {
          estimatedTime.value = `${seconds}s`
        }
      } else {
        estimatedTime.value = ''
      }
    }

    const startListening = async () => {
      unlisten = await listen(props.eventChannel, (event) => {
        if (props.eventChannel === 'installation-progress') {
          const { stage, percentage, message, detail, version } = event.payload

          if (message) {
            currentMessage.value = message
            if (detail) {
              currentMessage.value += ` - ${detail}`
            }
          } else if (stage) {
            currentMessage.value = t(`progress.stages.${stage}`)
          }

          if (percentage !== undefined) {
            progressPercentage.value = Math.min(100, Math.max(0, percentage))
            updateEstimatedTime()
          }

          const stageMapping = {
            'checking': { status: 'inProgress', step: 0 },
            'prerequisites': { status: 'inProgress', step: 1 },
            'download': { status: 'inProgress', step: 2 },
            'extract': { status: 'inProgress', step: 3 },
            'tools': { status: 'inProgress', step: 4 },
            'python': { status: 'inProgress', step: 5 },
            'configure': { status: 'inProgress', step: 6 },
            'complete': { status: 'completed', step: 7 },
            'error': { status: 'failed', step: currentStep.value }
          }

          if (stageMapping[stage]) {
            const { status, step } = stageMapping[stage]

            // Update status using translations
            statusText.value = t(`progress.status.${status}`)
            processing.value = status === 'inProgress'

            // Update step
            if (step !== undefined && props.steps.length > 0) {
              currentStep.value = Math.min(step, props.steps.length - 1)
            }
          }
        } else {
          // Legacy format handling
          const { message, percentage, status, step } = event.payload

          if (message !== undefined) {
            currentMessage.value = message
          }

          if (percentage !== undefined) {
            progressPercentage.value = Math.min(100, Math.max(0, percentage))
            updateEstimatedTime()
          }

          if (status !== undefined) {
            switch (status) {
              case 'success':
                statusText.value = t('progress.status.completed')
                processing.value = false
                progressPercentage.value = 100
                break
              case 'error':
                statusText.value = t('progress.status.failed')
                processing.value = false
                break
              case 'warning':
                statusText.value = t('progress.status.warning')
                break
              default:
                statusText.value = status
            }
          }

          if (step !== undefined && props.steps.length > 0) {
            currentStep.value = Math.min(step, props.steps.length - 1)
          }
        }
      })
    }

    onMounted(() => {
      startListening()

      // Update estimated time periodically
      if (props.showDetails) {
        progressInterval = setInterval(updateEstimatedTime, 1000)
      }
    })

    onUnmounted(() => {
      if (unlisten) {
        unlisten()
      }
      if (progressInterval) {
        clearInterval(progressInterval)
      }
    })

    return {
      currentMessage,
      progressPercentage,
      processing,
      currentStep,
      statusText,
      estimatedTime,
      progressClass,
      progressColor,
      railColor,
      getStepStatus
    }
  }
}
</script>

<style scoped>
.global-progress {
  padding: 1.5rem;
  background: white;
  border-radius: 8px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
}

.progress-content {
  display: flex;
  align-items: center;
  gap: 1.5rem;
}

.align-left .progress-content {
  justify-content: flex-start;
}

.align-center .progress-content {
  justify-content: center;
}

.align-right .progress-content {
  justify-content: flex-end;
  flex-direction: row-reverse;
}

.progress-spinner {
  flex-shrink: 0;
}

.progress-info {
  flex: 1;
  max-width: 600px;
}

.progress-info.has-spinner {
  max-width: 500px;
}

.progress-message {
  font-size: 1rem;
  color: #1f2937;
  margin-bottom: 0.75rem;
  font-family: 'Trueno-regular', sans-serif;
}

.align-center .progress-message {
  text-align: center;
}

.align-right .progress-message {
  text-align: right;
}

.progress-details {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 0.5rem;
  font-size: 0.875rem;
  color: #6b7280;
}

.progress-status {
  font-weight: 500;
}

.progress-time {
  font-family: monospace;
}

.progress-steps {
  margin-top: 2rem;
  padding-top: 2rem;
  border-top: 1px solid #e5e7eb;
}

/* Theme variations */
.theme-primary .progress-message {
  color: #1f2937;
}

.theme-success .progress-message {
  color: #065f46;
}

.theme-warning .progress-message {
  color: #92400e;
}

.theme-error .progress-message {
  color: #991b1b;
}

.theme-info .progress-message {
  color: #1e40af;
}

/* Responsive */
@media (max-width: 640px) {
  .progress-content {
    flex-direction: column;
    text-align: center;
  }

  .align-left .progress-content,
  .align-right .progress-content {
    flex-direction: column;
  }

  .progress-info {
    max-width: 100%;
  }

  .progress-details {
    flex-direction: column;
    gap: 0.25rem;
  }
}
</style>
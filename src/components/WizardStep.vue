<template>
  <div class="wizard-container">
    <h2>Step {{ currentStep }}: {{ stepTitle }}</h2>
    <div class="form-group">
      <!-- Add step-specific content here -->
      <label v-if="currentStep === 1" for="deviceName">Device Name:</label>
      <input v-if="currentStep === 1" id="deviceName" v-model="deviceName" placeholder="Enter device name">
      
      <label v-if="currentStep === 2" for="connectionType">Connection Type:</label>
      <select v-if="currentStep === 2" id="connectionType" v-model="connectionType">
        <option value="usb">USB</option>
        <option value="bluetooth">Bluetooth</option>
        <option value="wifi">Wi-Fi</option>
      </select>
      
      <label v-if="currentStep === 3" for="confirmInstall">Confirm Installation:</label>
      <n-checkbox v-if="currentStep === 3" id="confirmInstall" v-model:checked="confirmInstall">
        I confirm that the device is properly connected and ready for use.
      </n-checkbox>
    </div>
    <div>
      <n-button @click="previousStep" :disabled="currentStep === 1">Previous</n-button>
      <n-button @click="nextStep" :disabled="currentStep === totalSteps" type="primary">
        {{ currentStep === totalSteps ? 'Finish' : 'Next' }}
      </n-button>
    </div>
  </div>
</template>

<script>
import { ref, computed } from 'vue'
import { useWizardStore } from '../store'
import { NButton, NCheckbox } from 'naive-ui'

export default {
  components: { NButton, NCheckbox },
  setup() {
    const store = useWizardStore()
    const deviceName = ref('')
    const connectionType = ref('usb')
    const confirmInstall = ref(false)

    const stepTitle = computed(() => {
      switch (store.currentStep) {
        case 1: return 'Device Information'
        case 2: return 'Connection Setup'
        case 3: return 'Confirmation'
        default: return ''
      }
    })

    const nextStep = () => {
      if (store.currentStep === 1) {
        store.updateData({ deviceName: deviceName.value })
      } else if (store.currentStep === 2) {
        store.updateData({ connectionType: connectionType.value })
      } else if (store.currentStep === 3) {
        store.updateData({ confirmed: confirmInstall.value })
      }
      store.nextStep()
    }
    
    return {
      currentStep: computed(() => store.currentStep),
      totalSteps: computed(() => store.totalSteps),
      stepTitle,
      deviceName,
      connectionType,
      confirmInstall,
      nextStep,
      previousStep: store.previousStep
    }
  }
}
</script>
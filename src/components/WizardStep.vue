<template>
  <div class="wizard-container">
    <h2>Step {{ currentStep }}: {{ stepTitle }}</h2>
    <Welcome v-if="currentStep === 1" />
    <Greet v-if="currentStep === 2" />

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
import Welcome from './steps/Welcome.vue';
import Greet from './Greet.vue';



export default {
  components: { NButton, NCheckbox, Welcome, Greet },
  setup() {
    const store = useWizardStore()

    const steps = [{
      title: "Welcome",
    },
    {
      title: "Optional config loading",
    }
    ]; // Add more steps as needed

    const stepTitle = computed(() => {
      steps[store.currentStep - 1].title
    })


    const nextStep = () => {
      if (store.currentStep === 1) {
        // store.updateData({ deviceName: deviceName.value })
      } else if (store.currentStep === 2) {
        // store.updateData({ connectionType: connectionType.value })
      } else if (store.currentStep === 3) {
        // store.updateData({ confirmed: confirmInstall.value })
      }
      store.nextStep()
    }

    return {
      currentStep: computed(() => store.currentStep),
      totalSteps: computed(() => store.totalSteps),
      stepTitle,
      nextStep,
      previousStep: store.previousStep
    }
  }
}
</script>
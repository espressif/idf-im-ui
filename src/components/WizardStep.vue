<template>
  <div class="wizard-container">
    <h2>Step {{ currentStep }}: {{ stepTitle }}</h2>
    <PrerequisitiesCheck :nextstep=nextStep v-if="currentStep === 1" />
    <PythonSanitycheck :nextstep=nextStep v-if="currentStep === 2" />
    <TargetSelect :nextstep=nextStep v-if="currentStep === 3" />
    <VersionSelect :nextstep=nextStep v-if="currentStep === 4" />
    <MirrorSelect :nextstep=nextStep v-if="currentStep === 5" />
    <div>
      <!-- <n-button @click="previousStep" :disabled="currentStep === 1">Previous</n-button>
      <n-button @click="nextStep" :disabled="currentStep === totalSteps" type="primary">
        {{ currentStep === totalSteps ? 'Finish' : 'Next' }}
      </n-button> -->
    </div>
  </div>
</template>

<script>
import { ref, computed } from 'vue'
import { useWizardStore } from '../store'
import { NButton, NCheckbox } from 'naive-ui'
import Greet from './Greet.vue';
import PrerequisitiesCheck from './wizard_steps/PrerequisitiesCheck.vue';
import PythonSanitycheck from './wizard_steps/PythonSanitycheck.vue';
import TargetSelect from './wizard_steps/TargetSelect.vue';
import VersionSelect from './wizard_steps/VersionSelect.vue';
import MirrorSelect from './wizard_steps/MirrorSelect.vue';

export default {
  components: { NButton, NCheckbox, Greet, PrerequisitiesCheck, PythonSanitycheck, TargetSelect, VersionSelect, MirrorSelect },
  setup() {
    const store = useWizardStore()

    const steps = [
      {
        title: "Prerequisities Check ",
      },
      {
        title: "Python Sanity check",
      },
      {
        title: "Select Target",
      },
      {
        title: "Select IDF Version",
      },
      {
        title: "Select mirror",
      },
      {
        title: "Select Installation Path",
      },
      {
        title: "Install single version",
      },
      {
        title: "Post install steps", // creating desktop shorcut
      }
    ]; // Add more steps as needed

    const stepTitle = computed(() => {
      return steps[store.currentStep - 1].title
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
<template>
  <div class="wizard-layout" data-id="wizard-layout">
    <div class="wizard-sidebar" data-id="wizard-sidebar">
      <div class="steps-list" data-id="steps-list">
        <div v-for="(step, index) in steps" :key="index" class="step-item" :class="{
          'active': currentStep === index + 1,
          'completed': currentStep > index + 1,
          'disabled': currentStep === 7 || currentStep === 8,
          'clickable': currentStep > index + 1 && currentStep < 7
        }" @click="handleStepClick(index + 1)" :data-id="`step-item-${index + 1}`">
          <div class="step-number" :data-id="`step-number-${index + 1}`">
            <template v-if="currentStep > index + 1">
              <svg class="checkmark" viewBox="0 0 24 24" fill="none" stroke="currentColor" data-id="step-checkmark">
                <path d="M20 6L9 17L4 12" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
              </svg>
            </template>
            <template v-else>
              {{ index + 1 }}
            </template>
          </div>
          <div class="step-title" :data-id="`step-title-${index + 1}`">{{ step.title }}</div>
        </div>
      </div>
    </div>

    <!-- Main content -->
    <div class="wizard-container" data-id="wizard-container">
      <div class="wizard-step-container" data-id="wizard-step-content">
        <PrerequisitiesCheck :nextstep=nextStep v-if="currentStep === 1" data-id="prerequisites-check" />
        <PythonSanitycheck :nextstep=nextStep v-if="currentStep === 2" data-id="python-sanity-check" />
        <TargetSelect :nextstep=nextStep v-if="currentStep === 3" data-id="target-select" />
        <VersionSelect :nextstep=nextStep v-if="currentStep === 4" data-id="version-select" />
        <MirrorSelect :nextstep=nextStep v-if="currentStep === 5" data-id="mirror-select" />
        <InstallationPathSelect :nextstep=nextStep v-if="currentStep === 6" data-id="installation-path-select" />
        <InstalationProgress :nextstep=nextStep v-if="currentStep === 7" data-id="installation-progress" />
        <Complete v-if="currentStep === 8" data-id="complete" />
      </div>
    </div>
  </div>
</template>

<script>
import { ref, computed } from 'vue'
import { useWizardStore } from '../store'
import { NButton, NCheckbox } from 'naive-ui'
import PrerequisitiesCheck from './wizard_steps/PrerequisitiesCheck.vue';
import PythonSanitycheck from './wizard_steps/PythonSanitycheck.vue';
import TargetSelect from './wizard_steps/TargetSelect.vue';
import VersionSelect from './wizard_steps/VersionSelect.vue';
import MirrorSelect from './wizard_steps/MirrorSelect.vue';
import InstallationPathSelect from './wizard_steps/InstallationPathSelect.vue';
import InstalationProgress from './wizard_steps/InstalationProgress.vue';
import Complete from './wizard_steps/Complete.vue';


export default {
  name: 'WizardStep',
  components: {
    Complete,
    NButton,
    NCheckbox,
    PrerequisitiesCheck,
    PythonSanitycheck,
    TargetSelect,
    VersionSelect,
    MirrorSelect,
    InstallationPathSelect,
    InstalationProgress,
  },
  data() {
    return {
      steps: [
        { title: "Prerequisities Check" },
        { title: "Python Sanity check" },
        { title: "Select Target" },
        { title: "Select IDF Version" },
        { title: "Select mirror" },
        { title: "Select Installation Path" },
        { title: "Installation progress" },
        { title: "Instalation Complete" }
      ]
    }
  },
  computed: {
    store() {
      return useWizardStore()
    },
    currentStep() {
      return this.store.currentStep
    },
    totalSteps() {
      return this.store.totalSteps
    },
    stepTitle() {
      return this.steps[this.store.currentStep - 1].title
    }
  },
  methods: {
    initializeSteps() {
      this.steps = [
        { title: "Prerequisities Check" },
        { title: "Python Sanity check" },
        { title: "Select Target" },
        { title: "Select IDF Version" },
        { title: "Select mirror" },
        { title: "Select Installation Path" },
        { title: "Installation progress" },
        { title: "Instalation Complete" }
      ];
    },
    handleStepClick(stepNumber) {
      // Only allow navigation if:
      // 1. The step has been completed (currentStep > stepNumber)
      // 2. We're not in the installation or completion steps (currentStep < 7)
      // 3. We're not trying to navigate to a step after our current position
      if (this.currentStep > stepNumber && this.currentStep < 7) {
        this.store.setStep(stepNumber);
      }
    },
    nextStep() {
      if (this.store.currentStep === 1) {
        // this.store.updateData({ deviceName: deviceName.value })
      } else if (this.store.currentStep === 2) {
        // this.store.updateData({ connectionType: connectionType.value })
      } else if (this.store.currentStep === 3) {
        // this.store.updateData({ confirmed: confirmInstall.value })
      }
      this.store.nextStep()
    },
    previousStep() {
      this.store.previousStep()
    },
  },
  onBeforeMount() {
    this.store = useWizardStore();
    this.initializeSteps();
  }
}
</script>

<style scoped>
.wizard-layout {
  display: flex;
  min-height: 100vh;
}

.wizard-sidebar {
  width: 280px;
  background-color: #f5f5f5;
  padding: 20px;
  border-right: 1px solid #e0e0e0;
}

.steps-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.step-item {
  display: flex;
  align-items: center;
  padding: 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.3s ease;
  color: #666;
}

.step-item:hover:not(.disabled) {
  background-color: #e8e8e8;
}

.step-item.active {
  background-color: #ffeeed;
  color: #e7352c;
}

.step-item.completed {
  color: #666;
}

.step-item.disabled {
  cursor: not-allowed;
  opacity: 0.7;
}

.step-item.clickable {
  cursor: pointer;
}

.step-item.clickable:hover {
  background-color: #e8e8e8;
}

.step-number {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background-color: #fff;
  border: 1px solid currentColor;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-right: 12px;
  font-size: 14px;
  position: relative;
}

.checkmark {
  width: 16px;
  height: 16px;
}

.step-title {
  font-size: 14px;
  font-weight: 500;
}

.wizard-container {
  flex: 1;
  padding: 20px;
}

.wizard-step-container {
  max-width: 800px;
  margin: 0 auto;
}

/* Add smooth transitions */
.step-item {
  transition: all 0.3s ease;
}

.step-number {
  transition: all 0.3s ease;
}

/* Add subtle shadow to active step */
.step-item.active {
  box-shadow: 0 2px 4px rgba(231, 53, 44, 0.1);
}
</style>
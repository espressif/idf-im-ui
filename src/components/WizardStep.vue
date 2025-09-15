<template>
  <div class="wizard-layout" data-id="wizard-layout">
    <!-- Expert Installation Step Header -->
    <div class="wizard-header">
      <h1 class="header-title">Expert Installation Step</h1>
      <div class="step-indicator">Step {{ currentStep }}</div>
    </div>

    <div class="wizard-content">
      <!-- Moved sidebar -->
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

      <!-- Main content area -->
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
        { title: "Installation Complete" }
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
        this.store.goToStep(stepNumber);
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
  min-height: calc(100vh - 130px);
}

.wizard-header {
  padding: 24px 32px;
  padding-bottom: 0px;
  margin: 0 auto;
  max-width: 1136px;
  border-bottom: 1px solid #E5E7EB;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.header-title {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 36px;
  font-weight: 500;
  color: #111827;
}

.step-indicator {
  font-family: 'Trueno-bold', sans-serif;
  font-size: 48px;
  color: #e1e1e1;
  font-weight: 500;
}

.wizard-content {
  display: flex;
  padding: 0px;
  padding-top: 0px;
  ;
  gap: 32px;
  max-width: 1200px;
  margin: 0 auto;
  background-color: white;
  border-radius: 8px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

.wizard-sidebar {
  width: 280px;
  padding: 24px;

}

.steps-list {
  display: flex;
  flex-direction: column;
  position: relative;
}

.step-item {
  display: flex;
  align-items: flex-start;
  padding: 0 0 32px 0;
  position: relative;
  cursor: pointer;
  transition: all 0.3s ease;
}

/* Vertical line connecting steps */
.step-item:not(:last-child)::after {
  content: '';
  position: absolute;
  left: 16px;
  top: 32px;
  width: 2px;
  height: calc(100% - 32px);
  background-color: #E5E7EB;
  z-index: 1;
}

.step-item.completed:not(:last-child)::after {
  background-color: #3B82F6;
}

.step-number {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  background-color: white;
  border: 2px solid #E5E7EB;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-right: 16px;
  font-size: 17px;
  color: #6B7280;
  flex-shrink: 0;
  z-index: 2;
}

.step-item.active .step-number {
  border-color: #3B82F6;
  background-color: #3B82F6;
  color: white;
}

.step-item.completed .step-number {
  border-color: #3B82F6;
  background-color: #3B82F6;
  color: white;
}

.checkmark {
  width: 16px;
  height: 16px;
  stroke: white;
}

.step-title {
  font-size: 14px;
  padding-top: 4px;
  color: #6B7280;
}

.step-item.active .step-title {
  color: #3B82F6;
  font-weight: 500;
}

.step-item.completed .step-title {
  color: #374151;
}

.step-item.disabled {
  cursor: not-allowed;
  opacity: 0.6;
}

.step-item.clickable {
  cursor: pointer;
}

.step-item.clickable:hover .step-title {
  color: #3B82F6;
}

.wizard-container {
  flex: 1;
  padding: 24px;
}

.wizard-step-container {
  max-width: 800px;
  margin: 0 auto;
}

/* Transitions */
.step-item,
.step-number,
.step-title {
  transition: all 0.3s ease;
}
</style>

import { defineStore } from "pinia";

export const useWizardStore = defineStore("wizard", {
  state: () => ({
    currentStep: 1,
    totalSteps: 8,
    wizardData: {},
  }),
  actions: {
    nextStep() {
      if (this.currentStep < this.totalSteps) {
        this.currentStep++;
      }
    },
    setStep(step) {
      this.currentStep = step;
    },
    previousStep() {
      if (this.currentStep > 1) {
        this.currentStep--;
      }
    },
    updateData(data) {
      this.wizardData = { ...this.wizardData, ...data };
    },
  },
});

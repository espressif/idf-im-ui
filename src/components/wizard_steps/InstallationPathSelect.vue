<template>
  <div class="install-path" data-id="install-path">
    <h1 class="title" data-id="install-path-title">Select Installation Location</h1>

    <n-card class="path-card" data-id="path-selection-card">
      <div class="card-content" data-id="path-card-content">
        <div class="path-info" data-id="path-info-section">
          <h3 class="info-title" data-id="path-info-title">ESP-IDF Installation Directory</h3>
          <p class="info-desc" data-id="path-info-description">Choose where to install ESP-IDF and its tools. Ensure you
            have sufficient disk space.</p>
        </div>

        <div class="path-input" data-id="path-input-section">
          <n-input-group data-id="path-input-group">
            <n-input v-model:value="installPath" placeholder="Choose installation directory" class="path-field"
              data-id="installation-path-input" />
            <n-button @click="openFolderDialog" type="error" data-id="browse-button">
              Browse
            </n-button>
          </n-input-group>
        </div>

        <div class="path-validation" v-if="pathError" data-id="path-validation-section">
          <p :class="['error-message', 'error-message-' + pathIsValid]" data-id="path-error-message">{{ pathError }}</p>
        </div>
        <div v-if="pathSelected" class="path-validation" data-id="path-validation-section-succes">
          <p class="sucess-message" data-id="path-success-message">Instalation path updated successfully! </p>
        </div>
      </div>

      <div class="action-footer" data-id="path-action-footer">
        <n-button @click="processInstallPath" :disabled="!pathIsValid" type="error" size="large"
          data-id="continue-path-button">
          Continue
        </n-button>
      </div>
    </n-card>
  </div>
</template>
<script>
import { ref, onMounted, computed } from 'vue';
import { invoke } from "@tauri-apps/api/core";
import { open } from '@tauri-apps/plugin-dialog';
import { homeDir } from '@tauri-apps/api/path';
import { NButton, NInput, NInputGroup, NSpace, NCard } from 'naive-ui';
import { path } from '@tauri-apps/api';

export default {
  name: 'InstallPathSelect',
  props: {
    nextstep: Function
  },
  components: { NButton, NInput, NInputGroup, NSpace, NCard },
  data() {
    return {
      installPath: '',
      pathError: '',
      pathIsValid: false,
      pathSelected: false
    };
  },
  watch: {
    async installPath(newValue, oldValue) {
      console.log("installPath changed from", oldValue, "to", newValue);
      // This function will run every time installPath changes
      let result = await this.validatePath(newValue);
      if (!result) {
        this.pathError = `Path ${newValue} is not valid because it contains conflicting files or directories. Please choose a empty or non-existent directory.`;
        this.pathIsValid = false;
      } else {
        this.pathError = `Path ${newValue} is valid.`;
        this.pathIsValid = true;
      }
    }
  },
  computed: {
    async isValidPath() {
      console.log("Validating path:", path);
      return this.installPath.length > 0 && this.validatePath(this.installPath);
    }
  },
  methods: {
    async validatePath(path) {
      let result = await invoke("is_path_empty_or_nonexistent_command", { path: path });
      return result;
    },
    async openFolderDialog() {
      const selected = await open({
        directory: true,
        multiple: false,
      });
      if (selected) {
        this.installPath = await path.join(selected, '.espressif');
        this.pathSelected = true;
      }
    },
    async processInstallPath() {
      if (!this.isValidPath) {
        this.pathError = "Invalid path. Please choose a valid directory.";
        return;
      }
      console.log("Selected installation path:", this.installPath);
      await invoke("set_installation_path", { path: this.installPath });
      this.nextstep();
    }
  },
  async mounted() {
    const path = await invoke("get_installation_path");
    this.installPath = path;
  }
}
</script>

<style scoped>
.install-path {
  padding: 2rem;
  max-width: 800px;
  margin: 0 auto;
}

.title {
  font-size: 27px;
  font-family: 'Trueno-bold', sans-serif;
  color: #374151;
  margin-bottom: 0.5rem;
}

.path-card {
  background: white;
  padding: 1.5rem;
}

.card-content {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.path-info {
  margin-bottom: 1rem;
}

.info-title {
  font-size: 1.2rem;
  color: #374151;
  margin-bottom: 0.5rem;
}

.info-desc {
  color: #6b7280;
}

.space-required {
  display: flex;
  align-items: center;
  gap: 1rem;
  padding: 1rem;
  background: #fee2e2;
  border-radius: 0.5rem;
}

.space-icon {
  width: 2.5rem;
  height: 2.5rem;
  color: #e7352c;
}

.space-text {
  display: flex;
  flex-direction: column;
}

.space-label {
  font-size: 0.875rem;
  color: #6b7280;
}

.space-value {
  font-size: 1.25rem;
  font-weight: 500;
  color: #374151;
}

.path-input {
  margin: 1rem 0;
}

.path-field {
  font-family: monospace;
}

.error-message {
  color: #e7352c;
  font-size: 0.875rem;
}

.sucess-message {
  color: #5AC8FA;
  font-size: 0.875rem;
}

.action-footer {
  display: flex;
  justify-content: center;
  margin-top: 2rem;
  padding-top: 1rem;
}

.icon {
  width: 100%;
  height: 100%;
}

.n-button {
  background: #E8362D;
}

.n-card {
  border: none;
  border-top: 1px solid #e5e7eb;

}

.error-message {
  margin-left: 20%;
  margin-right: 20%;
  margin-bottom: 10px;
  padding: 10px;
}

.error-message-false {
  background-color: #fdeae8;
  border-left: 4px solid #E8362D;
}

.error-message-true {
  background-color: #eaf3fb;
  border-left: 4px solid #5AC8FA;
  color: #374151
}
</style>

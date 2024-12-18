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
          <p class="error-message" data-id="path-error-message">{{ pathError }}</p>
        </div>
      </div>

      <div class="action-footer" data-id="path-action-footer">
        <n-button @click="processInstallPath" type="error" size="large" data-id="continue-path-button">
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
  setup(props) {
    const installPath = ref('');
    const pathError = ref('');

    const isValidPath = computed(() => {
      return true; // TODO: add some validation logic here
      // return installPath.value.length > 0 && !pathError.value;
    });

    onMounted(async () => {
      const path = await invoke("get_installation_path");
      installPath.value = path;
    });

    const validatePath = (path) => {
      // Add path validation logic here if needed
      return true;
    };

    const openFolderDialog = async () => {
      const selected = await open({
        directory: true,
        multiple: false,
      });
      if (selected) {
        installPath.value = await path.join(selected, '.espressif');
      }
    };

    const processInstallPath = async () => {
      console.log("Selected installation path:", installPath.value);
      await invoke("set_installation_path", { path: installPath.value });
      props.nextstep();
    };

    return {
      installPath,
      openFolderDialog,
      processInstallPath
    };
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
  font-size: 1.8rem;
  color: #374151;
  margin-bottom: 2rem;
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

.action-footer {
  display: flex;
  justify-content: center;
  margin-top: 2rem;
  padding-top: 1rem;
  border-top: 1px solid #e5e7eb;
}

.icon {
  width: 100%;
  height: 100%;
}
</style>
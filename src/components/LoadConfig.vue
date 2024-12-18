<template>
  <div class="load-config" data-id="load-config">
    <h1 class="title" data-id="main-title">Installation Setup</h1>

    <!-- Compact Config Loading Section -->
    <div class="config-section" data-id="config-section" @dragover.prevent="handleDragOver"
      @dragleave.prevent="handleDragLeave" @drop.prevent="handleDrop" :class="{ 'dragging': isDragging }">
      <div class="config-content" data-id="config-content">
        <div class="config-layout" data-id="config-layout">
          <div class="config-text" data-id="config-text">
            <h2 class="section-title" data-id="config-section-title">Load Configuration</h2>
            <p class="section-description" data-id="config-description">Drag & drop TOML file or click to load existing
              configuration</p>
          </div>
          <div v-if="config_loaded" class="config-status" data-id="config-status">
            ✓ Config loaded
          </div>
          <n-button @click="load_config" type="error" ghost class="action-button" data-id="load-config-button">
            Load Config
          </n-button>
        </div>
      </div>
    </div>

    <!-- Installation Options -->
    <div class="installation-options" data-id="installation-options">
      <!-- Simplified Installation -->
      <div class="option-panel" data-id="simplified-installation-panel">
        <div class="option-content" data-id="simplified-installation-content">
          <div class="icon-container" data-id="simplified-icon-container">
            <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" data-id="simplified-icon">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
            </svg>
          </div>
          <h2 class="option-title" data-id="simplified-title">Simplified Installation</h2>
          <p class="option-description" data-id="simplified-description">
            The installer will take care of all the necessary steps for you, including installing the required
            dependencies, configuring the device, and installing the software.
          </p>
          <n-button @click="startSimplifiedSetup" type="error" ghost class="action-button"
            data-id="start-simplified-button">
            Start Simplified Setup
          </n-button>
        </div>
      </div>

      <!-- Expert Installation -->
      <div class="option-panel" data-id="expert-installation-panel">
        <div class="option-content" data-id="expert-installation-content">
          <div class="icon-container" data-id="expert-icon-container">
            <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" data-id="expert-icon">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
            </svg>
          </div>
          <h2 class="option-title" data-id="expert-title">Expert Installation</h2>
          <p class="option-description" data-id="expert-description">
            Let our wizard guide you through a streamlined installation process where you can configure every step.
          </p>
          <n-button @click="startWizard" type="error" ghost class="action-button" data-id="start-expert-button">
            Start Expert Setup
          </n-button>
        </div>
      </div>
    </div>
  </div>
</template>

<script>
import { open } from '@tauri-apps/plugin-dialog';
import { NButton } from 'naive-ui'
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { emit } from '@tauri-apps/api/event'

export default {
  name: 'LoadConfig',
  components: { NButton },
  data: () => ({
    rust_settings: {},
    unlisten_drag_drop: undefined,
    isDragging: false,
    config_loaded: false
  }),
  methods: {
    startWizard() {
      this.$router.push('/wizard/1');
    },
    startSimplifiedSetup() {
      this.$router.push('/simple-setup');
    },
    async gs() {
      this.rust_settings = await invoke("get_settings", {});
      return false;
    },
    async load_config() {
      console.log('Loading config...');
      const file = await open({
        title: 'Select installation config file',
        multiple: false,
        directory: false,
        filters: [
          { name: '*', extensions: ['toml'] },
        ],
      });
      const _ = await invoke("load_settings", { path: file });
      this.gs();
      this.config_loaded = true;
    }
  },
  mounted: async function () {
    this.gs();
    this.unlisten_drag_drop = await getCurrentWebview().onDragDropEvent(async (event) => {
      if (event.payload.type === 'over') {
        this.isDragging = true;
      } else if (event.payload.type === 'drop') {
        this.isDragging = false;

        console.log('User dropped', event.payload.paths);
        const file = event.payload.paths[0];
        // biome-ignore lint/complexity/useOptionalChain: <explanation>
        if (file && file.endsWith('.toml')) {
          const _ = await invoke("load_settings", { path: file });
          this.gs();
          this.config_loaded = true;
        } else {
          emit('user-message', {
            type: 'error',
            message: 'Invalid file type. Please select a TOML file.'
          }
          )
        }
      } else {
        this.isDragging = false;
        console.log('File drop cancelled');
      }
    });
  },
  beforeDestroy() {
    if (this.unlisten_drag_drop) {
      this.unlisten_drag_drop();
    }
  }
}
</script>

<style scoped>
.load-config {
  padding: 2rem;
  max-width: 1200px;
  margin: 0 auto;
  flex: 1;
  align-items: center;
  justify-content: center;
}

.title {
  font-size: 2rem;
  color: #374151;
  margin-bottom: 1.5rem;
  text-align: center;
}

.config-section {
  margin-bottom: 2rem;
  background: white;
  border-radius: 8px;
  box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
  padding: 1rem 2rem;
  transition: all 0.3s ease;
  border: 2px solid transparent;
}

.config-section.dragging {
  background-color: #fee2e2;
  border-color: #e7352c;
}

.config-content {
  width: 100%;
}

.config-layout {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
}

.config-text {
  text-align: left;
}

.section-title {
  font-size: 1.2rem;
  color: #374151;
  margin: 0;
}

.section-description {
  color: #6b7280;
  margin: 0.25rem 0 0 0;
  font-size: 0.9rem;
}

.installation-options {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 2rem;
  margin-top: 2rem;
}

.option-panel {
  background: white;
  border-radius: 8px;
  box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
  padding: 2rem;
  height: 100%;
}

.option-content {
  text-align: center;
}

.icon-container {
  background-color: #fee2e2;
  width: 64px;
  height: 64px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  margin: 0 auto 1.5rem;
}

.icon {
  width: 32px;
  height: 32px;
  color: #e7352c;
}

.option-title {
  font-size: 1.5rem;
  color: #374151;
  margin-bottom: 1rem;
}

.option-description {
  color: #6b7280;
  margin-bottom: 1.5rem;
  line-height: 1.5;
}

.action-button {
  white-space: nowrap;
}

.config-status {
  color: #059669;
  font-weight: 500;
  white-space: nowrap;
}

@media (max-width: 768px) {
  .installation-options {
    grid-template-columns: 1fr;
  }

  .config-layout {
    flex-direction: column;
    text-align: center;
    gap: 0.5rem;
    padding: 0.5rem 0;
  }

  .config-text {
    text-align: center;
  }
}
</style>
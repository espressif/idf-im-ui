<template>
  <div class="load-config">
    <h1 class="title">Please select starting point!</h1>
    <n-split direction="horizontal" class="content-split">
      <template #1>
        <div class="option-panel">
          <div class="option-content" @dragover.prevent="handleDragOver" @dragleave.prevent="handleDragLeave"
            @drop.prevent="handleDrop" :class="{ 'dragging': isDragging }">
            <div class="drop-zone">
              <div class="icon-container">
                <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                    d="M5 19a2 2 0 01-2-2V7a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1M5 19h14a2 2 0 002-2v-5a2 2 0 00-2-2H9a2 2 0 00-2 2v5a2 2 0 01-2 2z" />
                </svg>
              </div>
              <h2 class="option-title">Load Configuration</h2>
              <p class="option-description">Load an existing configuration file to start the installation process</p>
              <p class="drag-drop-text">
                <span class="drag-icon">📄</span>
                Drag and drop your TOML file here
                <br>
                <span class="drag-drop-or">or</span>
              </p>
              <n-button @click="load_config" type="error" ghost class="action-button">
                Load Installation Config
              </n-button>
            </div>
            <div v-if="Object.keys(rust_settings).length > 0" class="config-preview">
              <h3 class="preview-title">Current Configuration:</h3>
              <pre>{{ JSON.stringify(rust_settings, null, 2) }}</pre>
            </div>
          </div>
        </div>
      </template>
      <template #2>
        <div class="option-panel">
          <div class="option-content">
            <div class="icon-container">
              <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                  d="M19.428 15.428a2 2 0 00-1.022-.547l-2.387-.477a6 6 0 00-3.86.517l-.318.158a6 6 0 01-3.86.517L6.05 15.21a2 2 0 00-1.806.547M8 4h8l-1 1v5.172a2 2 0 00.586 1.414l5 5c1.26 1.26.367 3.414-1.415 3.414H4.828c-1.782 0-2.674-2.154-1.414-3.414l5-5A2 2 0 009 10.172V5L8 4z" />
              </svg>
            </div>
            <h2 class="option-title">Installation Wizard</h2>
            <p class="option-description">Let our wizard guide you through the installation process step by step</p>
            <n-button @click="startWizard" type="error" ghost class="action-button">
              Start Wizard
            </n-button>
          </div>
        </div>
      </template>
    </n-split>
  </div>
</template>

<script>
import { open } from '@tauri-apps/plugin-dialog';
import { NSplit, NButton } from 'naive-ui'
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";

export default {
  name: 'LoadConfig',
  components: { NSplit, NButton },
  data: () => ({
    rust_settings: {},
    unlisten_drag_drop: undefined,
    isDragging: false
  }),
  methods: {
    startWizard() {
      this.$router.push('/wizard/1');
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
  font-size: 1.8rem;
  color: #374151;
  margin-bottom: 2rem;
  text-align: center;
}

.content-split {
  min-height: 550px;
  background: white;
  border-radius: 8px;
  box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
}

.option-panel {
  height: 100%;
  padding: 2rem;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: top;
}

.option-content {
  text-align: center;
  max-width: 400px;
  transition: all 0.3s ease;
}

.drop-zone {
  padding: 2rem;
  border: 2px dashed #e5e7eb;
  border-radius: 8px;
  transition: all 0.3s ease;
}

.dragging .drop-zone {
  border-color: #e7352c;
  background-color: #fee2e2;
}

.drag-drop-text {
  margin: 1rem 0;
  color: #6b7280;
  font-size: 0.9rem;
}

.drag-icon {
  font-size: 1.5rem;
  display: block;
  margin-bottom: 0.5rem;
}

.drag-drop-or {
  display: inline-block;
  padding: 0.5rem;
  color: #9ca3af;
  font-size: 0.8rem;
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
  min-width: 200px;
}

.config-preview {
  margin-top: 2rem;
  text-align: left;
  background: #f3f4f6;
  padding: 1rem;
  border-radius: 4px;
  max-height: 200px;
  overflow-y: auto;
}

.preview-title {
  font-size: 0.875rem;
  color: #374151;
  margin-bottom: 0.5rem;
}

.config-preview pre {
  font-size: 0.75rem;
  color: #4b5563;
  white-space: pre-wrap;
}
</style>
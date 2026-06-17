<template>
  <n-modal
    v-model:show="showModal"
    preset="card"
    :title="t('app.incompleteInstallations.modalTitle')"
    style="width: 640px"
    :bordered="false"
    data-id="incomplete-installations-modal"
  >
    <div class="modal-body">
      <p class="description">{{ t('app.incompleteInstallations.description') }}</p>

      <div
        v-for="inst in installations"
        :key="inst.id"
        class="installation-row"
        :data-id="`incomplete-installation-${inst.id}`"
      >
        <div class="installation-info">
          <span class="installation-name">{{ inst.name }}</span>
          <n-tag :type="statusTagType(inst.status)" size="small" class="status-tag">
            {{ statusLabel(inst.status) }}
          </n-tag>
          <span class="installation-path">{{ inst.path || t('app.incompleteInstallations.noPath') }}</span>
        </div>
        <div class="installation-actions">
          <n-button
            size="small"
            type="primary"
            :disabled="deletingId === inst.id"
            @click="handleFix(inst)"
            :data-id="`fix-incomplete-${inst.id}`"
          >
            {{ t('app.incompleteInstallations.fixButton') }}
          </n-button>
          <n-button
            size="small"
            type="error"
            :loading="deletingId === inst.id"
            :disabled="!!deletingId"
            @click="handleDelete(inst)"
            :data-id="`delete-incomplete-${inst.id}`"
          >
            {{ t('app.incompleteInstallations.deleteButton') }}
          </n-button>
        </div>
      </div>
    </div>

    <template #footer>
      <div class="modal-footer">
        <n-button @click="showModal = false" data-id="dismiss-incomplete-modal">
          {{ t('app.incompleteInstallations.dismissButton') }}
        </n-button>
      </div>
    </template>
  </n-modal>
</template>

<script>
import { ref, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { NModal, NButton, NTag, useMessage } from 'naive-ui'

export default {
  name: 'IncompleteInstallationsNotification',
  components: { NModal, NButton, NTag },
  setup() {
    const { t } = useI18n()
    const message = useMessage()
    const router = useRouter()

    const showModal = ref(false)
    const installations = ref([])
    const deletingId = ref(null)

    const statusLabel = (status) => {
      const map = {
        in_progress: t('app.incompleteInstallations.statusInProgress'),
        failed: t('app.incompleteInstallations.statusFailed'),
        being_repaired: t('app.incompleteInstallations.statusBeingRepaired'),
        broken: t('app.incompleteInstallations.statusBroken'),
      }
      return map[status] ?? status
    }

    const statusTagType = (status) => {
      const map = {
        in_progress: 'warning',
        failed: 'error',
        being_repaired: 'warning',
        broken: 'error',
      }
      return map[status] ?? 'default'
    }

    const checkIncomplete = async () => {
      try {
        const result = await invoke('check_incomplete_installations')
        if (result && result.length > 0) {
          installations.value = result
          showModal.value = true
        }
      } catch (e) {
        console.log('Incomplete installation check failed:', e)
      }
    }

    const removeFromList = (id) => {
      installations.value = installations.value.filter(i => i.id !== id)
      if (installations.value.length === 0) {
        showModal.value = false
      }
    }

    const handleFix = (inst) => {
      showModal.value = false
      // Fire and forget — progress is tracked on the installation progress page
      invoke('fix_installation', { id: inst.id }).catch((e) => {
        message.error(`Failed to start repair: ${e}`)
      })
      message.success(t('app.incompleteInstallations.fixStarted', { name: inst.name }))
      router.push({
        path: '/installation-progress',
        query: {
          mode: 'fix',
          id: inst.id,
          name: inst.name,
          path: inst.path,
          autotrack: 'true',
        },
      })
    }

    const handleDelete = async (inst) => {
      deletingId.value = inst.id
      try {
        await invoke('remove_installation', { id: inst.id })
        removeFromList(inst.id)
        window.dispatchEvent(new CustomEvent('installations-changed'))
      } catch (e) {
        message.error(`Failed to delete ${inst.name}: ${e}`)
      } finally {
        deletingId.value = null
      }
    }

    onMounted(() => {
      checkIncomplete()
    })

    return {
      t,
      showModal,
      installations,
      deletingId,
      statusLabel,
      statusTagType,
      handleFix,
      handleDelete,
    }
  },
}
</script>

<style scoped>
.modal-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.description {
  margin: 0 0 8px;
  color: var(--n-text-color-2);
}

.installation-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
  border: 1px solid var(--n-border-color);
  border-radius: 6px;
}

.installation-info {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex: 1;
  min-width: 0;
}

.installation-name {
  font-weight: 600;
  display: flex;
  align-items: center;
  gap: 8px;
}

.status-tag {
  align-self: flex-start;
}

.installation-path {
  font-size: 0.8em;
  color: var(--n-text-color-3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.installation-actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
}

.slide-up-enter-active,
.slide-up-leave-active {
  transition: transform 0.3s ease, opacity 0.3s ease;
}

.slide-up-enter-from,
.slide-up-leave-to {
  transform: translateY(100%);
  opacity: 0;
}
</style>

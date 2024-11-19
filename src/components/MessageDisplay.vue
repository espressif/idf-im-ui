<template>
  <div class="message-display">
    <n-message-provider placement="top-right">
      <message-consumer />
    </n-message-provider>
  </div>
</template>

<script>
import { NMessageProvider } from 'naive-ui'
import { defineComponent, h } from 'vue'
import { useMessage } from 'naive-ui'
import { listen } from '@tauri-apps/api/event'

const MessageConsumer = defineComponent({
  setup() {
    const message = useMessage()

    // listen('user-message', (event) => {
    //   console.log('Received message:', event)
    //   message[event.payload.type](event.payload.message, { duration: 10000 })
    // })

    listen('user-message', (event) => {
      const duration = event.payload.type === 'error' ? 0 : 10000
      const options = {
        duration,
        closable: event.payload.type === 'error',
        keepAliveOnHover: true
      }

      message[event.payload.type](event.payload.message, options)
    })

    return () => null
  }
})

export default defineComponent({
  components: {
    NMessageProvider,
    MessageConsumer
  }
})
</script>

<style scoped>
.message-display {
  position: fixed;
  top: 20px;
  right: 20px;
  z-index: 1000;
  pointer-events: none;
}

:deep(.n-message) {
  pointer-events: auto;
}
</style>
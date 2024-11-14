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

    listen('user-message', (event) => {
      console.log('Received message:', event)
      message[event.payload.type](event.payload.message, { duration: 10000 })
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
}
</style>
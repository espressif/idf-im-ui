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

const MessageConsumer = defineComponent({
  setup() {
    const message = useMessage()

    // Simulating listening to a channel
    // Replace this with your actual channel listening logic
    setInterval(() => {
      const types = ['info', 'success', 'warning', 'error']
      const type = types[Math.floor(Math.random() * types.length)]
      message[type](`This is a ${type} message`)
    }, 5000)

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
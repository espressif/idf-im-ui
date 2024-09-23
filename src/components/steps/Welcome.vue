<template>
  <div class="welcome">
    <h1>Welcome to our app!</h1>
    <p>This is a sample welcome message.</p>
    <button @click="sayHello">Say Hello</button>
  </div>
</template>

<script>
export default {
  name: 'Welcome',
  methods: {
    sayHello() {
      console.log('Hello!');
    }
  }
}
</script>

<style scoped>
.welcome {
  text-align: center;
  padding: 20px;
  border: 1px solid #ccc;
  border-radius: 10px;
  box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);
}
</style>
<script setup>
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

const greetMsg = ref("");
const name = ref("");

async function greet(e) {
  e.preventDefault();
  // Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
  greetMsg.value = await invoke("greet", { name: name.value });
  return false;
}

async function gs(e) {
  e.preventDefault();
  // Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
  console.log(await invoke("get_settings", {}));
  return false;
}
</script>

<template>
  <form class="row">
    <input id="greet-input" v-model="name" placeholder="Enter a name..." />
    <button @click="greet">Greet</button>
    <button @click="gs">Get Settings</button>
  </form>

  <p>{{ greetMsg }}</p>
</template>

<script setup lang="ts">
import { RouterView } from "vue-router";
import { onMounted } from "vue";
import AppShell from "@/components/AppShell.vue";
import SessionRestore from "@/components/SessionRestore.vue";
import { useAppStore } from "@/stores/app";

const store = useAppStore();
onMounted(() => {
  void store.boot();
});
</script>

<template>
  <AppShell>
    <RouterView />
  </AppShell>
  <SessionRestore v-if="store.showSessionDialog" />
</template>

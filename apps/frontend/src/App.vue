<script setup lang="ts">
import { RouterView } from "vue-router";
import { onMounted } from "vue";
import AppShell from "@/components/AppShell.vue";
import OnboardingWizard from "@/components/OnboardingWizard.vue";
import SessionRestore from "@/components/SessionRestore.vue";
import { useAppStore } from "@/stores/app";
import { useReverseStore } from "@/stores/reverse";

const store = useAppStore();
const reverse = useReverseStore();
onMounted(() => {
  void store.boot();
  void reverse.boot();
});
</script>

<template>
  <AppShell>
    <RouterView />
  </AppShell>
  <OnboardingWizard v-if="store.ready && !store.settings.hasOnboarded" />
  <SessionRestore v-else-if="store.showSessionDialog" />
</template>

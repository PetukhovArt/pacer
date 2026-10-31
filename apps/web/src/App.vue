<script setup lang="ts">
import { useClientState } from '@core/vue-bindings';
import LoginView from './LoginView.vue';
import WorkspaceView from './WorkspaceView.vue';

const state = useClientState();
</script>

<template>
  <WorkspaceView v-if="state.me" :key="state.me.id" />
  <LoginView v-else />
</template>

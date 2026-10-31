<script setup lang="ts">
import { onMounted, ref } from 'vue';
import type { AuthConfig } from '@veyra/protocol';
import { useClient } from '@core/vue-bindings';
import { VButton, VInputText, VMessage } from '@ui/kit';

const client = useClient();
const name = ref(new URLSearchParams(location.search).get('as') ?? '');
const error = ref('');
const config = ref<AuthConfig | null>(null);

onMounted(async () => {
  try {
    config.value = await client.authConfig();
  } catch (e) {
    error.value = (e as Error).message;
  }
});

async function login() {
  error.value = '';
  try {
    await client.login(name.value.trim());
  } catch (e) {
    error.value = (e as Error).message;
  }
}
</script>

<template>
  <form class="login" @submit.prevent="login">
    <h1>Veyra</h1>
    <p class="vr-muted">Пространство, где команда думает вместе</p>
    <VButton v-if="config?.oidc" as="a" href="/api/auth/oidc/login" label="Войти через SSO" data-testid="login-sso" />
    <template v-if="config?.devLogin">
      <span class="vr-muted dev">вход для разработки</span>
      <VInputText v-model="name" data-testid="login-name" placeholder="Ваше имя" aria-label="Ваше имя" autofocus />
      <VButton type="submit" data-testid="login-submit" label="Войти" :disabled="!name.trim()" />
    </template>
    <VMessage v-if="error" severity="error">{{ error }}</VMessage>
  </form>
</template>

<style scoped>
.login {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
  max-width: 320px;
  margin: 15vh auto;
  padding: var(--vr-space-6);
  background: var(--vr-color-surface);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
}

h1,
p {
  margin: 0;
}

.dev {
  font-size: 0.85em;
}
</style>

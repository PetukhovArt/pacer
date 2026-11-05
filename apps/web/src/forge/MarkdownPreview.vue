<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue';
import { describeError, forgeFile } from '../connectors/api';
import RichContent from '../content/RichContent.vue';
import { t } from '../i18n';
import { fileBaseUrl } from './fileBase';

const props = defineProps<{ connector: string; project: string; sha: string; path: string; requestUrl: string }>();

const text = shallowRef<string | null>(null);
const failure = shallowRef<string | null>(null);
const baseUrl = computed(() => fileBaseUrl(props.requestUrl, props.sha, props.path));

let generation = 0;

async function load() {
  const ticket = ++generation;
  text.value = null;
  failure.value = null;
  try {
    const content = await forgeFile(props.connector, props.project, props.sha, props.path);
    if (ticket === generation) text.value = content;
  } catch (e) {
    if (ticket === generation) failure.value = describeError(e);
  }
}

watch(() => [props.connector, props.project, props.sha, props.path], load, { immediate: true });
</script>

<template>
  <div class="preview" data-testid="diff-md-preview">
    <p v-if="failure" class="note failure" role="alert">{{ failure }}</p>
    <p v-else-if="text === null" class="note">{{ t('forge.preview.loading') }}</p>
    <RichContent v-else :markdown="text" :connector="connector" :base-url="baseUrl" variant="document" :breaks="false" />
  </div>
</template>

<style scoped>
.preview {
  padding: var(--vr-space-4) var(--vr-space-6);
}

.note {
  margin: 0;
  color: var(--vr-color-text-muted);
}

.failure {
  color: var(--vr-color-danger);
}
</style>

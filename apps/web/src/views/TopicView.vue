<script setup lang="ts">
import { computed, onScopeDispose, ref } from 'vue';
import { ChevronRight } from 'lucide-vue-next';
import { VMessage } from '@ui/kit';
import { useClient, useClientState } from '@core/vue-bindings';
import Composer from '../Composer.vue';
import TaskFeed from './TaskFeed.vue';

const props = defineProps<{ topicId: string }>();
const client = useClient();
const state = useClientState();
onScopeDispose(client.openTopic(props.topicId));

const topic = computed(() => state.value.topics.find((t) => t.id === props.topicId));
const events = computed(() => state.value.events.get(props.topicId) ?? []);
const feed = { client, state, events };
const workspaceName = computed(() => state.value.workspaces.find((w) => w.id === state.value.workspaceId)?.name ?? '');
const text = ref('');
const error = ref('');

async function send() {
  error.value = '';
  try {
    await client.sendMessage(props.topicId, text.value.trim());
    text.value = '';
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  }
}
</script>

<template>
  <div v-if="topic" class="topic" :data-testid="`topic-panel-${topic.name}`">
    <header>
      <nav class="crumbs" aria-label="Путь">
        <span>{{ workspaceName }}</span>
        <ChevronRight :size="12" />
        <span>Топики</span>
      </nav>
      <h1><span class="hash">#</span>{{ topic.name }}</h1>
    </header>
    <VMessage v-if="error" severity="error" closable class="error" @close="error = ''">{{ error }}</VMessage>
    <TaskFeed :ctx="feed" />
    <div class="dock">
      <Composer
        v-model="text"
        :placeholder="`Сообщение в #${topic.name}`"
        label="Сообщение в топик"
        testid="topic-text"
        send-testid="topic-send"
        send-label="Отправить"
        @submit="send"
      />
    </div>
  </div>
  <div v-else class="missing vr-muted">Топик не найден в этой компании.</div>
</template>

<style scoped>
.topic {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
}

header {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  padding: var(--vr-space-3) var(--vr-space-5);
  border-bottom: 1px solid var(--vr-color-border);
}

.crumbs {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
}

h1 {
  margin: 0;
  font-size: var(--vr-font-size-lg);
  font-weight: 600;
  line-height: 24px;
  letter-spacing: -0.01em;
}

.hash {
  margin-right: var(--vr-space-1);
  color: var(--vr-color-text-muted);
}

.error {
  margin: var(--vr-space-2) var(--vr-space-4) 0;
}

.dock {
  padding: 0 var(--vr-space-4) var(--vr-space-4);
}

.missing {
  padding: var(--vr-space-6);
}
</style>

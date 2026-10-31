<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import type { IDockviewPanelProps } from 'dockview-core';
import type { PinKind, VeyraEvent } from '@veyra/protocol';
import { useClient, useClientState } from '@core/vue-bindings';
import { VButton, VMessage, VSelect, VTag, VTextarea } from '@ui/kit';
import type { TaskPanelParams } from '../open-task';

const props = defineProps<{ params: IDockviewPanelProps<TaskPanelParams> }>();
const taskId = props.params.params.taskId;
const client = useClient();
const state = useClientState();

const task = computed(() => state.value.tasks.find((t) => t.id === taskId));
const field = computed(() => state.value.fields.get(taskId));
const events = computed(() => [...(state.value.events.get(taskId) ?? [])].reverse());

const pinKinds: { label: string; value: PinKind }[] = [
  { label: 'Решение', value: 'decision' },
  { label: 'Контракт', value: 'contract' },
  { label: 'Учесть', value: 'caveat' },
];
const pinKind = ref<PinKind>('decision');
const pinText = ref('');
const prompt = ref('');
const error = ref('');

onMounted(() => client.loadTask(taskId).catch(report));

function report(e: unknown) {
  error.value = (e as Error).message;
}

async function run(action: () => Promise<unknown>) {
  error.value = '';
  await action().catch(report);
}

async function addPin() {
  await run(() => client.pin(taskId, pinKind.value, pinText.value.trim()));
  pinText.value = '';
}

async function startAgent() {
  await run(() => client.startAgent(taskId, prompt.value.trim()));
  prompt.value = '';
}

function statusOf(actorId: string) {
  const actor = state.value.actors.get(actorId);
  return actor?.kind === 'agent' ? actor.status : state.value.presence.get(actorId) ? 'online' : 'offline';
}

function describe(event: VeyraEvent): string {
  switch (event.type) {
    case 'agent.message':
      return event.text;
    case 'agent.tool_call':
      return `${event.tool} ${event.summary}`;
    case 'agent.file_changed':
      return `изменён ${event.path}`;
    case 'agent.status':
      return `статус: ${event.status}${event.detail ? ` — ${event.detail}` : ''}`;
    case 'agent.cost':
      return `стоимость $${event.usd.toFixed(4)}`;
    case 'pin.created':
      return `пин [${event.pin.kind}]: ${event.pin.text}`;
    case 'task.assigned':
      return 'взял задачу';
    case 'agent.started':
      return `агент запущен: ${event.prompt}`;
  }
}
</script>

<template>
  <div v-if="task" class="task-panel" :data-testid="`task-panel-${task.externalRef}`">
    <header>
      <h2>{{ task.externalRef }} · {{ task.title }}</h2>
      <span class="vr-muted">
        {{ task.status }} · {{ task.assigneeId ? `исполнитель: ${client.actorName(task.assigneeId)}` : 'не назначена' }}
      </span>
      <VButton v-if="task.assigneeId !== state.me?.id" size="small" label="Взять себе" data-testid="assign-me" @click="run(() => client.assignToMe(taskId))" />
    </header>

    <VMessage v-if="error" severity="error" closable @close="error = ''">{{ error }}</VMessage>

    <section v-for="overlap in field?.overlaps ?? []" :key="overlap.module" class="overlap" data-testid="overlap">
      Пересечение в модуле <code>{{ overlap.module }}</code>: {{ overlap.actorIds.map((id) => client.actorName(id)).join(', ') }}
    </section>

    <section>
      <h3>Пины</h3>
      <ul class="pins" data-testid="pins">
        <li v-for="pin in field?.pins ?? []" :key="pin.id">
          <VTag :value="pinKinds.find((k) => k.value === pin.kind)?.label" />
          <span>{{ pin.text }}</span>
          <span class="vr-muted">— {{ client.actorName(pin.authorId) }}</span>
        </li>
      </ul>
      <form class="row" @submit.prevent="addPin">
        <VSelect v-model="pinKind" :options="pinKinds" option-label="label" option-value="value" aria-label="Тип пина" />
        <VTextarea v-model="pinText" rows="1" auto-resize placeholder="Что всем нужно учесть" aria-label="Текст пина" data-testid="pin-text" />
        <VButton type="submit" label="Закрепить" :disabled="!pinText.trim()" data-testid="pin-submit" />
      </form>
    </section>

    <section>
      <h3>Кто работает</h3>
      <ul class="activities" data-testid="activities">
        <li v-for="a in field?.activities ?? []" :key="a.actorId" :data-testid="`activity-${statusOf(a.actorId)}`">
          <span class="vr-dot" :data-status="statusOf(a.actorId)" :data-online="statusOf(a.actorId) === 'online'" />
          <strong>{{ client.actorName(a.actorId) }}</strong>
          <span class="vr-muted">{{ statusOf(a.actorId) }} · событий: {{ a.eventCount }}</span>
          <code v-for="m in a.modules" :key="m">{{ m }}</code>
        </li>
      </ul>
    </section>

    <section>
      <h3>Запустить агента</h3>
      <form class="row" @submit.prevent="startAgent">
        <VTextarea v-model="prompt" rows="2" auto-resize placeholder="Что сделать агенту" aria-label="Задание агенту" data-testid="agent-prompt" />
        <VButton type="submit" label="Запустить" :disabled="!prompt.trim()" data-testid="agent-start" />
      </form>
    </section>

    <section>
      <h3>Лента</h3>
      <ol class="feed" data-testid="feed">
        <li v-for="event in events" :key="event.id">
          <span class="vr-muted">{{ new Date(event.at).toLocaleTimeString() }}</span>
          <strong>{{ client.actorName(event.actorId) }}</strong>
          <span>{{ describe(event) }}</span>
        </li>
      </ol>
    </section>
  </div>
</template>

<style scoped>
.task-panel {
  container-type: inline-size;
  height: 100%;
  overflow: auto;
  padding: var(--vr-space-4);
  box-sizing: border-box;
  background: var(--vr-color-surface);
  color: var(--vr-color-text);
}

header {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-2);
}

h2 {
  flex-basis: 100%;
  margin: 0;
  font-size: 18px;
}

h3 {
  margin: var(--vr-space-4) 0 var(--vr-space-2);
  font-size: 13px;
  text-transform: uppercase;
  color: var(--vr-color-text-muted);
}

ul,
ol {
  list-style: none;
  margin: 0;
  padding: 0;
}

li {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-2);
  padding: var(--vr-space-1) 0;
}

.pins li {
  padding: var(--vr-space-2);
  margin-bottom: var(--vr-space-1);
  background: var(--vr-color-pin-bg);
  border-radius: var(--vr-radius-sm);
}

.overlap {
  margin-top: var(--vr-space-3);
  padding: var(--vr-space-2);
  border-left: 4px solid var(--vr-color-conflict);
  background: var(--vr-color-surface-muted);
}

.row {
  display: flex;
  gap: var(--vr-space-2);
  align-items: flex-start;
}

.row > :deep(textarea) {
  flex: 1;
}

.feed li {
  font-size: 13px;
  border-bottom: 1px solid var(--vr-color-border);
}

code {
  font-family: var(--vr-font-mono);
  font-size: 12px;
}

@container (max-width: 480px) {
  .row {
    flex-direction: column;
    align-items: stretch;
  }
}
</style>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import DOMPurify from 'dompurify';
import { marked } from 'marked';
import type { IDockviewPanelProps } from 'dockview-core';
import type { BillingMode, PinKind, VeyraEvent } from '@veyra/protocol';
import { useClient, useClientState } from '@core/vue-bindings';
import { VButton, VMessage, VSelect, VTag, VTextarea } from '@ui/kit';
import type { TaskPanelParams } from '../open-task';
import TaskMeta from './TaskMeta.vue';

const props = defineProps<{ params: IDockviewPanelProps<TaskPanelParams> }>();
const taskId = props.params.params.taskId;
const client = useClient();
const state = useClientState();

const task = computed(() => state.value.tasks.find((t) => t.id === taskId));
const field = computed(() => state.value.fields.get(taskId));
const messages = computed(() => (state.value.events.get(taskId) ?? []).flatMap((e) => (e.type === 'chat.message' ? [e.message] : [])));
const chatText = ref('');
const events = computed(() => [...(state.value.events.get(taskId) ?? [])].reverse());

type ToolCall = Extract<VeyraEvent, { type: 'agent.tool_call' }>;
type FeedItem = { kind: 'event'; id: string; event: VeyraEvent } | { kind: 'tools'; id: string; actorId: string; at: string; calls: ToolCall[] };

const feed = computed(() =>
  events.value.reduce<FeedItem[]>((items, event) => {
    const last = items[items.length - 1];
    if (event.type !== 'agent.tool_call') items.push({ kind: 'event', id: event.id, event });
    else if (last?.kind === 'tools' && last.actorId === event.actorId) last.calls.unshift(event);
    else items.push({ kind: 'tools', id: event.id, actorId: event.actorId, at: event.at, calls: [event] });
    return items;
  }, []),
);

function markdown(text: string): string {
  return DOMPurify.sanitize(marked.parse(text, { async: false, gfm: true, breaks: true }));
}

const pinKinds: { label: string; value: PinKind }[] = [
  { label: 'Решение', value: 'decision' },
  { label: 'Контракт', value: 'contract' },
  { label: 'Учесть', value: 'caveat' },
];
const pinKind = ref<PinKind>('decision');
const pinText = ref('');
const prompt = ref('');
const billingModes: { label: string; value: BillingMode }[] = [
  { label: 'Личная подписка', value: 'personal' },
  { label: 'Командный API', value: 'team' },
];
const billingMode = ref<BillingMode>('personal');
const projects = computed(() => state.value.workspaces.find((w) => w.id === state.value.workspaceId)?.projects ?? []);
const projectKey = computed(() => `veyra.agentProject.${state.value.workspaceId ?? 'default'}`);
const storedProject = ref(localStorage.getItem(projectKey.value));
const projectId = computed({
  get: () => (projects.value.some((p) => p.id === storedProject.value) ? storedProject.value : (projects.value[0]?.id ?? null)),
  set: (id: string | null) => {
    storedProject.value = id;
    if (id) localStorage.setItem(projectKey.value, id);
  },
});

function projectName(id: string | null | undefined) {
  return projects.value.find((p) => p.id === id)?.name ?? id ?? '';
}

function agentOf(actorId: string) {
  const actor = state.value.actors.get(actorId);
  return actor?.kind === 'agent' ? actor : null;
}

function worktreeOf(actorId: string) {
  const running = (state.value.events.get(taskId) ?? []).filter((e) => e.actorId === actorId && e.type === 'agent.status' && e.status === 'running');
  const last = running[running.length - 1];
  return last?.type === 'agent.status' ? (last.detail ?? '') : '';
}

const taskMergeRequests = computed(() => (task.value ? state.value.mergeRequests.filter((m) => m.taskRefs.includes(task.value!.externalRef)) : []));

function openMergeRequest(event: MouseEvent, id: string) {
  if (!('__TAURI_INTERNALS__' in window)) return;
  event.preventDefault();
  void run(() => client.openMergeRequest(id));
}

function openInTracker(event: MouseEvent) {
  if (!('__TAURI_INTERNALS__' in window)) return;
  event.preventDefault();
  void run(() => client.openTaskInTracker(taskId));
}

async function stopAgent(agentId: string) {
  await run(() => client.stopAgent(agentId));
}

const replyTexts = ref<Record<string, string>>({});

function canReply(actorId: string) {
  const agent = agentOf(actorId);
  return !!agent && agent.ownerId === state.value.me?.id && ['done', 'failed', 'stopped'].includes(agent.status);
}

async function replyToAgent(agentId: string) {
  const text = (replyTexts.value[agentId] ?? '').trim();
  if (!text) return;
  await run(async () => {
    await client.replyToAgent(agentId, text);
    replyTexts.value[agentId] = '';
  });
}
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
  await run(() => client.startAgent(taskId, prompt.value.trim(), billingMode.value, projectId.value));
  prompt.value = '';
}

async function sendChat() {
  await run(() => client.sendMessage(taskId, chatText.value.trim()));
  chatText.value = '';
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
    case 'task.updated':
      return `трекер: ${event.title} (${event.status})`;
    case 'task.mr':
      return `MR ${event.state}: ${event.title} ${event.url}`;
    case 'chat.message':
      return `в чате: ${event.message.text}`;
    case 'agent.reply':
      return `ответ агенту ${client.actorName(event.agentId)}: ${event.text}`;
    case 'agent.session':
      return `сессия ${event.sessionId}`;
    case 'agent.step':
      return `jig · ${event.unit}: ${event.status}`;
    case 'agent.todos':
      return `план: ${event.items.filter((i) => i.status === 'completed').length}/${event.items.length}`;
  }
}
</script>

<template>
  <div v-if="task" class="task-panel" :data-testid="`task-panel-${task.externalRef}`">
    <header>
      <h2 :title="task.title">
        <a v-if="task.webUrl" :href="task.webUrl" target="_blank" rel="noopener" class="key" data-testid="task-tracker-link" @click="openInTracker">{{ task.externalRef }}</a>
        <span v-else class="key">{{ task.externalRef }}</span>
        · {{ task.title }}
      </h2>
      <TaskMeta :task="task" />
      <span class="vr-muted">
        {{ task.status }} · {{ task.assigneeId ? `исполнитель: ${client.actorName(task.assigneeId)}` : task.assigneeName ? `исполнитель: ${task.assigneeName}` : 'не назначена' }}
      </span>
      <VButton v-if="task.assigneeId !== state.me?.id" size="small" label="Взять себе" data-testid="assign-me" @click="run(() => client.assignToMe(taskId))" />
    </header>

    <VMessage v-if="error" severity="error" closable @close="error = ''">{{ error }}</VMessage>

    <section v-if="field?.summary" class="summary" data-testid="task-summary">
      <h3>Сводка</h3>
      <p>{{ field.summary.text }}</p>
      <span class="vr-muted">обновлено {{ new Date(field.summary.at).toLocaleTimeString() }}</span>
    </section>

    <section v-for="overlap in field?.overlaps ?? []" :key="overlap.module" class="overlap" data-testid="overlap">
      Пересечение в модуле <code>{{ overlap.module }}</code>: {{ overlap.actorIds.map((id) => client.actorName(id)).join(', ') }}
    </section>

    <section v-if="taskMergeRequests.length">
      <h3>Merge requests</h3>
      <ul class="merge-requests" data-testid="merge-requests">
        <li v-for="mr in taskMergeRequests" :key="mr.id" :data-testid="`merge-request-${mr.iid}`">
          <a :href="mr.webUrl" target="_blank" rel="noopener" data-testid="merge-request-link" @click="openMergeRequest($event, mr.id)">!{{ mr.iid }} {{ mr.title }}</a>
          <VTag v-if="mr.draft" value="Draft" severity="secondary" />
          <span class="vr-muted">{{ mr.state }} · {{ projectName(mr.projectId) }}</span>
          <code>{{ mr.sourceBranch }} → {{ mr.targetBranch }}</code>
        </li>
      </ul>
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
          <span v-if="agentOf(a.actorId)" class="session" data-testid="agent-session">
            <span v-if="agentOf(a.actorId)?.projectId" data-testid="agent-session-project">проект: {{ projectName(agentOf(a.actorId)?.projectId) }}</span>
            <code data-testid="agent-session-branch">agent/{{ a.actorId.slice(0, 8) }}</code>
            <code v-if="worktreeOf(a.actorId)" data-testid="agent-session-worktree">{{ worktreeOf(a.actorId) }}</code>
            <VButton
              v-if="agentOf(a.actorId)?.status === 'running' && agentOf(a.actorId)?.ownerId === state.me?.id"
              size="small"
              severity="danger"
              text
              label="Остановить"
              data-testid="agent-stop"
              @click="stopAgent(a.actorId)"
            />
          </span>
          <form v-if="agentOf(a.actorId)?.ownerId === state.me?.id" class="row reply" @submit.prevent="replyToAgent(a.actorId)">
            <VTextarea
              v-model="replyTexts[a.actorId]"
              rows="1"
              auto-resize
              placeholder="Ответ агенту"
              aria-label="Ответ агенту"
              :disabled="!canReply(a.actorId)"
              data-testid="agent-reply-text"
            />
            <VButton type="submit" size="small" label="Ответить" :disabled="!canReply(a.actorId) || !replyTexts[a.actorId]?.trim()" data-testid="agent-reply-send" />
          </form>
        </li>
      </ul>
    </section>

    <section>
      <h3>Запустить агента</h3>
      <form class="row" @submit.prevent="startAgent">
        <VTextarea v-model="prompt" rows="2" auto-resize placeholder="Что сделать агенту" aria-label="Задание агенту" data-testid="agent-prompt" />
        <VSelect v-model="billingMode" :options="billingModes" option-label="label" option-value="value" aria-label="Оплата агента" data-testid="agent-billing" />
        <VSelect
          v-if="projects.length"
          v-model="projectId"
          :options="projects"
          option-label="name"
          option-value="id"
          aria-label="Проект агента"
          data-testid="agent-project"
        />
        <VButton type="submit" label="Запустить" :disabled="!prompt.trim()" data-testid="agent-start" />
      </form>
    </section>

    <section>
      <h3>Чат</h3>
      <ol class="chat" data-testid="chat">
        <li v-for="m in messages" :key="m.id">
          <strong>{{ client.actorName(m.authorId) }}</strong>
          <span class="vr-muted">{{ new Date(m.at).toLocaleTimeString() }}</span>
          <span class="chat-text">{{ m.text }}</span>
        </li>
      </ol>
      <form class="row" @submit.prevent="sendChat">
        <VTextarea v-model="chatText" rows="1" auto-resize placeholder="Сообщение по задаче" aria-label="Сообщение в чат" data-testid="chat-text" />
        <VButton type="submit" label="Отправить" :disabled="!chatText.trim()" data-testid="chat-send" />
      </form>
    </section>

    <section>
      <h3>Лента</h3>
      <ol class="feed" data-testid="feed">
        <li v-for="item in feed" :key="item.id" :class="item.kind">
          <template v-if="item.kind === 'tools'">
            <span class="vr-muted">{{ new Date(item.at).toLocaleTimeString() }}</span>
            <strong>{{ client.actorName(item.actorId) }}</strong>
            <span class="vr-muted">инструменты: {{ item.calls.length }}</span>
            <div class="tool-calls">
              <div v-for="call in item.calls" :key="call.id" class="tool-call">
                <code class="tool-name">{{ call.tool }}</code>
                <span class="tool-summary" :title="call.summary">{{ call.summary }}</span>
              </div>
            </div>
          </template>
          <template v-else>
            <span class="vr-muted">{{ new Date(item.event.at).toLocaleTimeString() }}</span>
            <strong>{{ client.actorName(item.event.actorId) }}</strong>
            <div v-if="item.event.type === 'agent.message'" class="markdown" v-html="markdown(item.event.text)" />
            <span v-else>{{ describe(item.event) }}</span>
          </template>
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
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  overflow: hidden;
}

.key {
  font-family: var(--vr-font-mono);
  color: var(--vr-color-accent);
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

.summary {
  margin-top: var(--vr-space-3);
  padding: var(--vr-space-2);
  border-left: 4px solid var(--vr-color-border);
  background: var(--vr-color-surface-muted);
}

.summary h3 {
  margin-top: 0;
}

.summary p {
  margin: 0 0 var(--vr-space-1);
  white-space: pre-wrap;
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

.session {
  display: flex;
  flex-basis: 100%;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-2);
  font-size: 12px;
}

.reply {
  flex-basis: 100%;
}

.chat-text {
  flex-basis: 100%;
  white-space: pre-wrap;
}

.feed li {
  font-size: 13px;
  border-bottom: 1px solid var(--vr-color-border);
  white-space: pre-wrap;
}

.feed li.tools {
  white-space: normal;
}

.tool-calls {
  display: flex;
  flex-direction: column;
  flex-basis: 100%;
  gap: 2px;
}

.tool-call {
  display: flex;
  gap: var(--vr-space-2);
  min-width: 0;
}

.tool-name {
  flex: none;
  font-weight: 600;
}

.tool-summary {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  color: var(--vr-color-text-muted);
  font-family: var(--vr-font-mono);
  font-size: 12px;
}

.markdown {
  flex-basis: 100%;
  white-space: normal;
  overflow-wrap: anywhere;
}

.markdown :deep(p),
.markdown :deep(ul),
.markdown :deep(ol),
.markdown :deep(pre) {
  margin: 0 0 var(--vr-space-1);
}

.markdown :deep(ul),
.markdown :deep(ol) {
  padding-left: var(--vr-space-4);
  list-style: revert;
}

.markdown :deep(li) {
  display: list-item;
  padding: 0;
}

.markdown :deep(pre) {
  overflow: auto;
  padding: var(--vr-space-2);
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-sm);
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

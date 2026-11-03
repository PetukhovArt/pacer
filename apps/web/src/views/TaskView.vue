<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import type { BillingMode } from '@veyra/protocol';
import { Bot, ChevronRight, ExternalLink, GitMerge, MessageSquare, Square } from 'lucide-vue-next';
import { VButton, VMessage, VSelect } from '@ui/kit';
import Avatar from '../Avatar.vue';
import Composer from '../Composer.vue';
import { view } from '../nav';
import TaskFeed from './TaskFeed.vue';
import TaskSide from './TaskSide.vue';
import { useTask } from './task';

const props = defineProps<{ taskId: string }>();
const ctx = useTask(props.taskId);
const { client, state, error, task, field, mergeRequests, projects, inTauri, projectName, agentOf, statusOf, lastOf, worktreeOf, run, openMergeRequest } = ctx;

const tab = ref<'feed' | 'agents' | 'mr'>('feed');
const mode = ref<'chat' | 'agent'>('chat');
const chatText = ref('');
const prompt = ref('');

const presets = [
  { label: 'jig: фича', value: 'feature', text: (key: string, title: string) => `/jig:jig feature: ${key} ${title}` },
  { label: 'jig: баг', value: 'bug', text: (key: string, title: string) => `/jig:jig bug-fix: ${key} ${title}` },
  {
    label: 'Анализ (без правок)',
    value: 'analysis',
    text: (key: string, title: string) =>
      `Проанализируй задачу ${key} «${title}» без правок кода: найди затронутые модули, опиши причину или план решения, риски и открытые вопросы. Файлы не меняй, ничего не коммить.`,
  },
];
const preset = ref<string | null>(null);

function applyPreset(value: string) {
  const picked = presets.find((p) => p.value === value);
  if (picked && task.value) prompt.value = picked.text(task.value.externalRef, task.value.title);
}

const billingModes: { label: string; value: BillingMode }[] = [
  { label: 'Личная подписка', value: 'personal' },
  { label: 'Командный API', value: 'team' },
];
const billingMode = ref<BillingMode>('personal');
const projectKey = `veyra.agentProject.${state.value.workspaceId ?? 'default'}`;
const storedProject = ref(localStorage.getItem(projectKey));
const projectId = computed({
  get: () => (projects.value.some((p) => p.id === storedProject.value) ? storedProject.value : (projects.value[0]?.id ?? null)),
  set: (id: string | null) => {
    storedProject.value = id;
    if (id) localStorage.setItem(projectKey, id);
  },
});

const todoIcons = { completed: '☑', in_progress: '◐', pending: '☐' } as const;
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

onMounted(() => run(() => client.loadTask(props.taskId)));

function openInTracker(event: MouseEvent) {
  if (!inTauri) return;
  event.preventDefault();
  void run(() => client.openTaskInTracker(props.taskId));
}

async function startAgent() {
  await run(() => client.startAgent(props.taskId, prompt.value.trim(), billingMode.value, projectId.value));
  prompt.value = '';
}

async function sendChat() {
  await run(() => client.sendMessage(props.taskId, chatText.value.trim()));
  chatText.value = '';
}

const workspaceName = () => state.value.workspaces.find((w) => w.id === state.value.workspaceId)?.name ?? '';
</script>

<template>
  <div v-if="task" class="task" :data-testid="`task-panel-${task.externalRef}`">
    <header>
      <nav class="crumbs" aria-label="Путь">
        <span>{{ workspaceName() }}</span>
        <ChevronRight :size="12" />
        <button @click="view = { kind: 'tasks' }">Задачи</button>
        <ChevronRight :size="12" />
        <span>{{ task.externalRef }}</span>
      </nav>
      <div class="title-row">
        <h1 :title="task.title">
          <a v-if="task.webUrl" :href="task.webUrl" target="_blank" rel="noopener" class="key" data-testid="task-tracker-link" title="Открыть в трекере" @click="openInTracker">
            {{ task.externalRef }}<ExternalLink :size="12" />
          </a>
          <span v-else class="key">{{ task.externalRef }}</span>
          <span>{{ task.title }}</span>
        </h1>
        <VButton v-if="task.assigneeId !== state.me?.id" size="small" label="Взять себе" data-testid="assign-me" @click="run(() => client.assignToMe(taskId))" />
      </div>
      <div class="vr-pills" role="tablist">
        <button role="tab" :aria-selected="tab === 'feed'" data-testid="tab-feed" @click="tab = 'feed'">Лента</button>
        <button role="tab" :aria-selected="tab === 'agents'" data-testid="tab-agents" @click="tab = 'agents'">Агенты {{ field?.activities.filter((a) => agentOf(a.actorId)).length || '' }}</button>
        <button role="tab" :aria-selected="tab === 'mr'" data-testid="tab-mr" @click="tab = 'mr'">MR {{ mergeRequests.length || '' }}</button>
      </div>
    </header>

    <div class="split">
      <div class="main">
        <VMessage v-if="error" severity="error" closable class="error" @close="error = ''">{{ error }}</VMessage>

        <TaskFeed v-if="tab === 'feed'" :ctx="ctx">
          <section v-if="field?.summary" class="banner" data-testid="task-summary">
            <strong>Сводка</strong>
            <p>{{ field.summary.text }}</p>
            <span class="vr-muted">обновлено {{ new Date(field.summary.at).toLocaleTimeString() }}</span>
          </section>
          <section v-for="overlap in field?.overlaps ?? []" :key="overlap.module" class="banner conflict" data-testid="overlap">
            Пересечение в модуле <code class="vr-code">{{ overlap.module }}</code>: {{ overlap.actorIds.map((id) => client.actorName(id)).join(', ') }}
          </section>
        </TaskFeed>

        <div v-else-if="tab === 'agents'" class="pane">
          <p v-if="!field?.activities.length" class="vr-muted">Никто ещё не работал над задачей. Запустите агента из ленты.</p>
          <ul class="vr-list" data-testid="activities">
            <li v-for="a in field?.activities ?? []" :key="a.actorId" class="activity" :data-testid="`activity-${statusOf(a.actorId)}`">
              <div class="who">
                <Avatar :id="a.actorId" :name="client.actorName(a.actorId)" :agent="!!agentOf(a.actorId)" small />
                <strong>{{ client.actorName(a.actorId) }}</strong>
                <span class="vr-dot" :data-status="statusOf(a.actorId)" :data-online="statusOf(a.actorId) === 'online'" />
                <span class="vr-muted">{{ statusOf(a.actorId) }} · событий: {{ a.eventCount }}</span>
                <VButton
                  v-if="agentOf(a.actorId)?.status === 'running' && agentOf(a.actorId)?.ownerId === state.me?.id"
                  class="stop"
                  size="small"
                  severity="danger"
                  text
                  label="Остановить"
                  data-testid="agent-stop"
                  @click="run(() => client.stopAgent(a.actorId))"
                >
                  <template #icon><Square :size="12" /></template>
                </VButton>
              </div>
              <div v-if="a.modules.length" class="vr-row-meta">
                <code v-for="m in a.modules" :key="m" class="vr-code">{{ m }}</code>
              </div>
              <div v-if="agentOf(a.actorId)" class="vr-row-meta" data-testid="agent-session">
                <span v-if="agentOf(a.actorId)?.projectId" data-testid="agent-session-project">проект: {{ projectName(agentOf(a.actorId)?.projectId) }}</span>
                <code class="vr-code" data-testid="agent-session-branch">agent/{{ a.actorId.slice(0, 8) }}</code>
                <code v-if="worktreeOf(a.actorId)" class="vr-code" data-testid="agent-session-worktree">{{ worktreeOf(a.actorId) }}</code>
              </div>
              <div v-if="agentOf(a.actorId) && (lastOf(a.actorId, 'agent.todos') || lastOf(a.actorId, 'agent.step'))" class="progress" data-testid="agent-progress">
                <ul v-if="lastOf(a.actorId, 'agent.todos')?.items.length" class="todos" data-testid="agent-todos">
                  <li v-for="(todo, i) in lastOf(a.actorId, 'agent.todos')?.items" :key="i" :data-status="todo.status">
                    <span aria-hidden="true">{{ todoIcons[todo.status] }}</span>
                    <strong v-if="todo.status === 'in_progress'">{{ todo.content }}</strong>
                    <span v-else>{{ todo.content }}</span>
                  </li>
                </ul>
                <span v-if="lastOf(a.actorId, 'agent.step')" class="vr-muted" data-testid="agent-jig-step">
                  jig · {{ lastOf(a.actorId, 'agent.step')?.unit }}: {{ lastOf(a.actorId, 'agent.step')?.status || lastOf(a.actorId, 'agent.step')?.phase }}
                </span>
              </div>
              <Composer
                v-if="agentOf(a.actorId)?.ownerId === state.me?.id"
                v-model="replyTexts[a.actorId]"
                class="reply"
                placeholder="Ответ агенту"
                label="Ответ агенту"
                testid="agent-reply-text"
                send-testid="agent-reply-send"
                send-label="Ответить"
                :disabled="!canReply(a.actorId)"
                @submit="replyToAgent(a.actorId)"
              />
            </li>
          </ul>
        </div>

        <div v-else class="pane">
          <p v-if="!mergeRequests.length" class="vr-muted">Merge requests с ключом {{ task.externalRef }} пока нет.</p>
          <ul v-else class="vr-list" data-testid="merge-requests">
            <li v-for="mr in mergeRequests" :key="mr.id" :data-testid="`merge-request-${mr.iid}`">
              <a class="vr-row mr" :href="mr.webUrl" target="_blank" rel="noopener" data-testid="merge-request-link" @click="openMergeRequest($event, mr.id)">
                <span class="who"><GitMerge :size="16" /><strong>!{{ mr.iid }} {{ mr.title }}</strong></span>
                <span class="vr-row-meta">
                  <span v-if="mr.draft" class="vr-chip">Draft</span>
                  <span>{{ mr.state }}</span>
                  <span>{{ projectName(mr.projectId) }}</span>
                  <code class="vr-code">{{ mr.sourceBranch }} → {{ mr.targetBranch }}</code>
                </span>
              </a>
            </li>
          </ul>
        </div>

        <div v-if="tab === 'feed'" class="dock">
          <Composer
            v-if="mode === 'chat'"
            v-model="chatText"
            :placeholder="`Сообщение в ${task.externalRef}`"
            label="Сообщение в чат"
            testid="chat-text"
            send-testid="chat-send"
            send-label="Отправить"
            @submit="sendChat"
          >
            <button type="button" class="mode" aria-pressed="true" title="Сообщение команде"><MessageSquare :size="16" /></button>
            <button type="button" class="mode" aria-pressed="false" title="Задание агенту" data-testid="composer-agent" @click="mode = 'agent'"><Bot :size="16" /></button>
          </Composer>
          <Composer
            v-else
            v-model="prompt"
            placeholder="Что сделать агенту"
            label="Задание агенту"
            testid="agent-prompt"
            send-testid="agent-start"
            send-label="Запустить агента"
            @submit="startAgent"
          >
            <button type="button" class="mode" aria-pressed="false" title="Сообщение команде" data-testid="composer-chat" @click="mode = 'chat'"><MessageSquare :size="16" /></button>
            <button type="button" class="mode" aria-pressed="true" title="Задание агенту"><Bot :size="16" /></button>
            <VSelect
              v-model="preset"
              :options="presets"
              option-label="label"
              option-value="value"
              placeholder="Шаблон"
              size="small"
              aria-label="Шаблон задания"
              data-testid="agent-preset"
              @update:model-value="applyPreset"
            />
            <VSelect v-model="billingMode" :options="billingModes" option-label="label" option-value="value" size="small" aria-label="Оплата агента" data-testid="agent-billing" />
            <VSelect
              v-if="projects.length"
              v-model="projectId"
              :options="projects"
              option-label="name"
              option-value="id"
              size="small"
              aria-label="Проект агента"
              data-testid="agent-project"
            />
          </Composer>
        </div>
      </div>
      <TaskSide :ctx="ctx" :task-id="taskId" />
    </div>
  </div>
  <div v-else class="missing vr-muted">Задача не найдена в этой компании.</div>
</template>

<style scoped>
.task {
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

.crumbs button {
  padding: 0;
  font: inherit;
  color: inherit;
  background: none;
  border: 0;
}

.crumbs button:hover {
  color: var(--vr-color-text);
}

.title-row {
  display: flex;
  align-items: flex-start;
  gap: var(--vr-space-3);
}

h1 {
  display: -webkit-box;
  flex: 1;
  margin: 0;
  font-size: var(--vr-font-size-lg);
  font-weight: 600;
  line-height: 24px;
  letter-spacing: -0.01em;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  overflow: hidden;
}

.key {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  margin-right: var(--vr-space-2);
  color: var(--vr-color-text-muted);
  text-decoration: none;
}

a.key:hover {
  color: var(--vr-color-text);
}

.split {
  display: flex;
  flex: 1;
  min-height: 0;
}

.main {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}

.error {
  margin: var(--vr-space-2) var(--vr-space-4) 0;
}

.banner {
  margin: 0 var(--vr-space-2) var(--vr-space-2);
  padding: var(--vr-space-2) var(--vr-space-3);
  border-radius: var(--vr-radius-lg);
  background: var(--vr-color-surface-muted);
}

.banner p {
  margin: 0;
  white-space: pre-wrap;
}

.banner.conflict {
  background: var(--vr-color-pin-bg);
}

.pane {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--vr-space-4) var(--vr-space-5);
}

.activity {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  padding: var(--vr-space-3);
}

.who {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-2);
}

.stop {
  margin-left: auto;
}

.progress {
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: var(--vr-font-size-xs);
}

.todos {
  margin: 0;
  padding: 0;
  list-style: none;
}

.todos li {
  display: flex;
  gap: var(--vr-space-1);
}

.todos li[data-status='completed'] {
  color: var(--vr-color-text-muted);
}

.reply {
  padding: var(--vr-space-2) var(--vr-space-3) var(--vr-space-1);
  border-radius: var(--vr-radius-xl);
}

.mr {
  color: inherit;
  text-decoration: none;
}

.mr:hover {
  background: var(--vr-color-surface-muted);
}

.dock {
  padding: 0 var(--vr-space-4) var(--vr-space-4);
}

.mode {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  padding: 0;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.mode:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-surface-muted);
}

.mode[aria-pressed='true'] {
  color: var(--vr-color-text);
  background: var(--vr-color-surface-muted);
}

.missing {
  padding: var(--vr-space-6);
}

@media (max-width: 900px) {
  .split {
    flex-direction: column;
  }

  .split > :deep(.side) {
    width: auto;
    max-height: 40%;
    border-left: 0;
    border-top: 1px solid var(--vr-color-border);
  }
}
</style>

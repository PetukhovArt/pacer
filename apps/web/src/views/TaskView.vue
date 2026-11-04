<script setup lang="ts">
import { computed, ref } from 'vue';
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
const { client, state, error, task, mergeRequests, agents, agentOf, lastOf, statusOf, worktreeOf, run } = ctx;

const tab = ref<'feed' | 'agents' | 'mr'>('feed');
const mode = ref<'chat' | 'agent'>('chat');
const chatText = ref('');
const prompt = ref('');

const agentKinds = client.runner?.agents ?? [];
const agentKind = ref(agentKinds[0] ?? null);
const projects = computed(() => client.runner?.projects.filter((p) => p.workspace === state.value.workspaceId) ?? []);
const pickedProject = ref<string | null>(null);
const projectId = computed({
  get: () => (projects.value.some((p) => p.id === pickedProject.value) ? pickedProject.value : (projects.value[0]?.id ?? null)),
  set: (id: string | null) => (pickedProject.value = id),
});

async function startAgent() {
  const agent = agentKind.value;
  const project = projectId.value;
  if (!agent || !project) {
    error.value = client.runner ? 'Выберите агента и проект' : 'Раннер не найден: запустите veyra-runner и перезапустите vite';
    return;
  }
  await run(async () => {
    await client.startAgent(props.taskId, { agent, projectId: project, prompt: prompt.value.trim() });
    prompt.value = '';
  });
}

const todoIcons = { completed: '☑', in_progress: '◐', pending: '☐' } as const;

async function sendChat() {
  await run(async () => {
    await client.sendMessage(props.taskId, chatText.value.trim());
    chatText.value = '';
  });
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
          <a v-if="task.webUrl" :href="task.webUrl" target="_blank" rel="noopener" class="key" data-testid="task-tracker-link" title="Открыть в трекере">
            {{ task.externalRef }}<ExternalLink :size="12" />
          </a>
          <span v-else class="key">{{ task.externalRef }}</span>
          <span>{{ task.title }}</span>
        </h1>
      </div>
      <div class="vr-pills" role="tablist">
        <button role="tab" :aria-selected="tab === 'feed'" data-testid="tab-feed" @click="tab = 'feed'">Лента</button>
        <button role="tab" :aria-selected="tab === 'agents'" data-testid="tab-agents" @click="tab = 'agents'">Агенты {{ agents.length || '' }}</button>
        <button role="tab" :aria-selected="tab === 'mr'" data-testid="tab-mr" @click="tab = 'mr'">MR {{ mergeRequests.length || '' }}</button>
      </div>
    </header>

    <div class="split">
      <div class="main">
        <VMessage v-if="error" severity="error" closable class="error" @close="error = ''">{{ error }}</VMessage>

        <TaskFeed v-if="tab === 'feed'" :ctx="ctx" />

        <div v-else-if="tab === 'agents'" class="pane">
          <p v-if="!agents.length" class="vr-muted">Агенты ещё не работали над задачей.</p>
          <ul class="vr-list" data-testid="activities">
            <li v-for="id in agents" :key="id" class="activity" :data-testid="`activity-${statusOf(id)}`">
              <div class="who">
                <Avatar :id="agentOf(id)?.actorId ?? id" :name="client.actorName(agentOf(id)?.actorId ?? id)" agent small />
                <strong>{{ agentOf(id)?.job?.agent || client.actorName(agentOf(id)?.actorId ?? id) }}</strong>
                <span class="vr-dot" :data-status="statusOf(id)" />
                <span class="vr-muted">{{ statusOf(id) }}</span>
                <VButton
                  v-if="statusOf(id) === 'running' && agentOf(id)?.job"
                  class="stop"
                  size="small"
                  severity="danger"
                  text
                  label="Остановить"
                  data-testid="agent-stop"
                  @click="run(() => client.stopAgent(taskId, id))"
                >
                  <template #icon><Square :size="12" /></template>
                </VButton>
              </div>
              <div v-if="worktreeOf(id)" class="vr-row-meta" data-testid="agent-session">
                <code class="vr-code" data-testid="agent-session-worktree">{{ worktreeOf(id) }}</code>
              </div>
              <div v-if="lastOf(id, 'agent.todos') || lastOf(id, 'agent.step')" class="progress" data-testid="agent-progress">
                <ul v-if="lastOf(id, 'agent.todos')?.items.length" class="todos" data-testid="agent-todos">
                  <li v-for="(todo, i) in lastOf(id, 'agent.todos')?.items" :key="i" :data-status="todo.status">
                    <span aria-hidden="true">{{ todoIcons[todo.status] }}</span>
                    <strong v-if="todo.status === 'in_progress'">{{ todo.content }}</strong>
                    <span v-else>{{ todo.content }}</span>
                  </li>
                </ul>
                <span v-if="lastOf(id, 'agent.step')" class="vr-muted" data-testid="agent-jig-step">
                  jig · {{ lastOf(id, 'agent.step')?.unit }}: {{ lastOf(id, 'agent.step')?.status || lastOf(id, 'agent.step')?.phase }}
                </span>
              </div>
            </li>
          </ul>
        </div>

        <div v-else class="pane">
          <p v-if="!mergeRequests.length" class="vr-muted">Merge requests с ключом {{ task.externalRef }} пока нет.</p>
          <ul v-else class="vr-list" data-testid="merge-requests">
            <li v-for="mr in mergeRequests" :key="mr.id" :data-testid="`merge-request-${mr.iid}`">
              <a class="vr-row mr" :href="mr.webUrl" target="_blank" rel="noopener" data-testid="merge-request-link">
                <span class="who"><GitMerge :size="16" /><strong>!{{ mr.iid }} {{ mr.title }}</strong></span>
                <span class="vr-row-meta">
                  <span v-if="mr.draft" class="vr-chip">Draft</span>
                  <span>{{ mr.state }}</span>
                  <span>{{ mr.projectId }}</span>
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
            <VSelect v-model="agentKind" :options="agentKinds" size="small" placeholder="Агент" aria-label="Агент" data-testid="agent-kind" />
            <VSelect
              v-model="projectId"
              :options="projects"
              option-label="name"
              option-value="id"
              size="small"
              placeholder="Проект"
              aria-label="Проект агента"
              data-testid="agent-project"
            />
          </Composer>
        </div>
      </div>
      <TaskSide :ctx="ctx" />
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

.stop {
  margin-left: auto;
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

.mode:hover,
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

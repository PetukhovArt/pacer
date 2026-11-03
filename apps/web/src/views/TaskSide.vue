<script setup lang="ts">
import { computed, ref } from 'vue';
import type { PinKind } from '@veyra/protocol';
import { VSelect } from '@ui/kit';
import Avatar from '../Avatar.vue';
import Composer from '../Composer.vue';
import TaskMeta from './TaskMeta.vue';
import type { TaskContext } from './task';

const props = defineProps<{ ctx: TaskContext; taskId: string }>();
const { client, state, task, field, mergeRequests, projectName, agentOf, statusOf, worktreeOf, run, openMergeRequest } = props.ctx;

const pinKinds: { label: string; value: PinKind }[] = [
  { label: 'Решение', value: 'decision' },
  { label: 'Контракт', value: 'contract' },
  { label: 'Учесть', value: 'caveat' },
];
const pinKind = ref<PinKind>('decision');
const pinText = ref('');

async function addPin() {
  await run(() => client.pin(props.taskId, pinKind.value, pinText.value.trim()));
  pinText.value = '';
}

const release = computed(() => state.value.releases.find((r) => r.id === task.value?.releaseId)?.name ?? 'Без релиза');
const agents = computed(() => (field.value?.activities ?? []).filter((a) => agentOf(a.actorId)));
const tone = computed(() => task.value?.statusCategory ?? 'todo');
</script>

<template>
  <aside v-if="task" class="side" aria-label="Сведения о задаче">
    <section>
      <h3 class="vr-section-title">Статус</h3>
      <span class="status" :data-tone="tone" data-testid="task-status">{{ task.status }}</span>
    </section>

    <section>
      <h3 class="vr-section-title">Исполнитель</h3>
      <div class="person">
        <template v-if="task.assigneeId || task.assigneeName">
          <Avatar :id="task.assigneeId ?? task.assigneeRef ?? ''" :name="task.assigneeId ? client.actorName(task.assigneeId) : task.assigneeName!" small />
          <span>{{ task.assigneeId ? client.actorName(task.assigneeId) : task.assigneeName }}</span>
        </template>
        <span v-else class="vr-muted">не назначена</span>
      </div>
    </section>

    <section v-if="task.priority || task.labels?.length">
      <h3 class="vr-section-title">Приоритет и метки</h3>
      <TaskMeta :task="task" />
    </section>

    <section>
      <h3 class="vr-section-title">Релиз</h3>
      <span>{{ release }}</span>
    </section>

    <section v-if="agents.length || mergeRequests.length">
      <h3 class="vr-section-title">Ветки</h3>
      <div v-for="a in agents" :key="a.actorId" class="branch">
        <code class="vr-code">agent/{{ a.actorId.slice(0, 8) }}</code>
        <code v-if="worktreeOf(a.actorId)" class="vr-code vr-muted">{{ worktreeOf(a.actorId) }}</code>
      </div>
      <div v-for="mr in mergeRequests" :key="mr.id" class="branch">
        <code class="vr-code">{{ mr.sourceBranch }}</code>
        <span class="vr-muted">→</span>
        <code class="vr-code">{{ mr.targetBranch }}</code>
      </div>
    </section>

    <section v-if="mergeRequests.length">
      <h3 class="vr-section-title">Merge requests</h3>
      <a v-for="mr in mergeRequests" :key="mr.id" class="mr" :href="mr.webUrl" target="_blank" rel="noopener" @click="openMergeRequest($event, mr.id)">
        <span class="status small" :data-tone="mr.state === 'merged' ? 'done' : mr.state === 'opened' ? 'in_progress' : 'todo'">{{ mr.draft ? 'draft' : mr.state }}</span>
        <span class="mr-title">!{{ mr.iid }} {{ mr.title }}</span>
      </a>
    </section>

    <section v-if="agents.length">
      <h3 class="vr-section-title">Агенты</h3>
      <div v-for="a in agents" :key="a.actorId" class="person">
        <Avatar :id="a.actorId" :name="client.actorName(a.actorId)" agent small />
        <span class="grow">{{ client.actorName(a.actorId) }}</span>
        <span class="vr-dot" :data-status="statusOf(a.actorId)" />
        <span class="vr-muted small-text">{{ statusOf(a.actorId) }}</span>
      </div>
      <span v-for="a in agents.filter((x) => agentOf(x.actorId)?.projectId)" :key="`p-${a.actorId}`" class="vr-muted small-text">проект: {{ projectName(agentOf(a.actorId)?.projectId) }}</span>
    </section>

    <section>
      <h3 class="vr-section-title">Пины</h3>
      <ul class="pins" data-testid="pins">
        <li v-for="pin in field?.pins ?? []" :key="pin.id">
          <span class="kind">{{ pinKinds.find((k) => k.value === pin.kind)?.label }}</span>
          <span class="pin-text">{{ pin.text }}</span>
          <span class="vr-muted small-text">{{ client.actorName(pin.authorId) }}</span>
        </li>
      </ul>
      <Composer v-model="pinText" placeholder="Что всем нужно учесть" label="Текст пина" testid="pin-text" send-testid="pin-submit" send-label="Закрепить" @submit="addPin">
        <VSelect v-model="pinKind" :options="pinKinds" option-label="label" option-value="value" size="small" aria-label="Тип пина" />
      </Composer>
    </section>
  </aside>
</template>

<style scoped>
.side {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-5);
  width: var(--vr-size-meta);
  flex: none;
  min-height: 0;
  overflow-y: auto;
  box-sizing: border-box;
  padding: var(--vr-space-4);
  border-left: 1px solid var(--vr-color-border);
  font-size: var(--vr-font-size-xs);
  line-height: 18px;
}

section {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--vr-space-1);
}

section > .vr-section-title {
  margin: 0;
}

.status {
  display: inline-flex;
  align-items: center;
  padding: 2px var(--vr-space-2);
  font-weight: 600;
  border-radius: var(--vr-radius-md);
  color: var(--vr-color-text);
  background: var(--vr-color-surface-muted);
}

.status[data-tone='in_progress'] {
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-success);
}

.status[data-tone='done'] {
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-text-muted);
}

.status.small {
  flex: none;
  padding: 0 6px;
  font-size: var(--vr-font-size-2xs);
}

.person {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  width: 100%;
}

.grow {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.branch {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-1);
}

.mr {
  display: flex;
  align-items: baseline;
  gap: var(--vr-space-2);
  width: 100%;
  color: inherit;
  text-decoration: none;
}

.mr:hover .mr-title {
  text-decoration: underline;
}

.small-text {
  font-size: var(--vr-font-size-2xs);
}

.pins {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
  width: 100%;
  margin: 0 0 var(--vr-space-1);
  padding: 0;
  list-style: none;
}

.pins li {
  display: flex;
  flex-wrap: wrap;
  gap: 2px var(--vr-space-2);
  padding: var(--vr-space-2);
  border-radius: var(--vr-radius-md);
  background: var(--vr-color-pin-bg);
}

.kind {
  font-weight: 600;
  color: var(--vr-color-conflict);
}

.pin-text {
  flex-basis: 100%;
  white-space: pre-wrap;
}

.side :deep(.composer) {
  width: 100%;
  box-sizing: border-box;
  padding: var(--vr-space-2) var(--vr-space-3) var(--vr-space-1);
  border-radius: var(--vr-radius-xl);
}
</style>

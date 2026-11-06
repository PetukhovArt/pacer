<script setup lang="ts">
import { computed } from 'vue';
import Avatar from '../Avatar.vue';
import { t } from '../i18n';
import TaskMeta from './TaskMeta.vue';
import type { TaskContext } from './task';

const props = defineProps<{ ctx: TaskContext }>();
const { client, state, task, mergeRequests, agents, statusOf, worktreeOf } = props.ctx;

const release = computed(() => state.value.releases.find((r) => r.id === task.value?.releaseId)?.name ?? t('tasks.noRelease'));
const tone = computed(() => task.value?.statusCategory ?? 'todo');
</script>

<template>
  <aside v-if="task" class="side" :aria-label="t('tasks.side.aria')">
    <section>
      <h3 class="vr-section-title">{{ t('tasks.side.status') }}</h3>
      <span class="status" :data-tone="tone" data-testid="task-status">{{ task.status }}</span>
    </section>

    <section>
      <h3 class="vr-section-title">{{ t('tasks.side.assignee') }}</h3>
      <div class="person">
        <template v-if="task.assigneeId">
          <Avatar :id="task.assigneeId" :name="client.actorName(task.assigneeId)" small />
          <span>{{ client.actorName(task.assigneeId) }}</span>
        </template>
        <span v-else class="vr-muted">{{ t('tasks.unassigned') }}</span>
      </div>
    </section>

    <section v-if="task.priority || task.labels?.length">
      <h3 class="vr-section-title">{{ t('tasks.side.priorityLabels') }}</h3>
      <TaskMeta :task="task" />
    </section>

    <section>
      <h3 class="vr-section-title">{{ t('tasks.side.release') }}</h3>
      <span>{{ release }}</span>
    </section>

    <section v-if="mergeRequests.length || agents.some(worktreeOf)">
      <h3 class="vr-section-title">{{ t('tasks.side.branches') }}</h3>
      <div v-for="id in agents.filter(worktreeOf)" :key="id" class="branch">
        <code class="vr-code vr-muted">{{ worktreeOf(id) }}</code>
      </div>
      <div v-for="mr in mergeRequests" :key="mr.id" class="branch">
        <code class="vr-code">{{ mr.sourceBranch }}</code>
        <span class="vr-muted">→</span>
        <code class="vr-code">{{ mr.targetBranch }}</code>
      </div>
    </section>

    <section v-if="mergeRequests.length">
      <h3 class="vr-section-title">{{ t('tasks.side.mergeRequests') }}</h3>
      <a v-for="mr in mergeRequests" :key="mr.id" class="mr" :href="mr.webUrl" target="_blank" rel="noopener">
        <span class="status small" :data-tone="mr.state === 'merged' ? 'done' : mr.state === 'opened' ? 'in_progress' : 'todo'">{{ mr.draft ? 'draft' : mr.state }}</span>
        <span class="mr-title">!{{ mr.iid }} {{ mr.title }}</span>
      </a>
    </section>

    <section v-if="agents.length">
      <h3 class="vr-section-title">{{ t('tasks.side.agents') }}</h3>
      <div v-for="id in agents" :key="id" class="person">
        <Avatar :id="id" :name="client.actorName(id)" agent small />
        <span class="grow">{{ client.actorName(id) }}</span>
        <span class="vr-dot" :data-status="statusOf(id)" />
        <span class="vr-muted small-text">{{ statusOf(id) }}</span>
      </div>
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





</style>

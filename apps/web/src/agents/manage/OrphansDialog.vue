<script setup lang="ts">
import { shallowRef, watch } from 'vue';
import { VButton, VDialog, VMessage, VSelect } from '@ui/kit';
import { describeError, usePacer } from '../pacer';
import { useAction } from '../useAction';
import { listOrphans, resumeOrphan, type OrphanedSession } from './orphans';
import { useWorktreeOptions } from './worktrees';

const props = defineProps<{ project: string }>();
const emit = defineEmits<{ resumed: [agent: string]; close: [] }>();

const dateFormat = new Intl.DateTimeFormat('ru-RU', { dateStyle: 'medium', timeStyle: 'short' });

const orphans = shallowRef<readonly OrphanedSession[]>([]);
const loading = shallowRef(true);
const loadError = shallowRef<string | null>(null);
const resuming = shallowRef<string | null>(null);
const resume = useAction();
const picks = shallowRef<Readonly<Record<string, string>>>({});
const options = useWorktreeOptions(() => props.project);
const pacer = usePacer();

let loadToken = 0;

async function load(project: string) {
  if (!pacer) return;
  const token = ++loadToken;
  loading.value = true;
  loadError.value = null;
  try {
    const list = await listOrphans(project);
    if (token === loadToken) orphans.value = list;
  } catch (e) {
    if (token !== loadToken) return;
    orphans.value = [];
    loadError.value = describeError(e);
  } finally {
    if (token === loadToken) loading.value = false;
  }
}

watch(() => props.project, load, { immediate: true });

function targetOf(session: string): string | undefined {
  const pick = picks.value[session];
  return options.value.find((option) => option.id === pick)?.id ?? options.value[0]?.id;
}

function pick(session: string, worktree: unknown) {
  if (typeof worktree === 'string') picks.value = { ...picks.value, [session]: worktree };
}

function startedAt(orphan: OrphanedSession): string | null {
  return orphan.created_at > 0 ? dateFormat.format(orphan.created_at) : null;
}

async function resumeSession(session: string) {
  const worktree = targetOf(session);
  if (!worktree || resume.busy.value) return;
  resuming.value = session;
  await resume.run(async () => {
    const agent = await resumeOrphan(session, worktree);
    orphans.value = orphans.value.filter((orphan) => orphan.session_id !== session);
    emit('resumed', agent);
  });
}
</script>

<template>
  <VDialog v-if="pacer" :visible="true" modal dismissable-mask header="Старые разговоры" :style="{ width: '46rem', maxWidth: '95vw' }" data-testid="orphans-dialog" @update:visible="emit('close')">
    <VMessage v-if="loadError" severity="error" size="small" data-testid="orphans-error">{{ loadError }}</VMessage>
    <p v-else-if="loading" class="state" role="status">Загрузка…</p>
    <p v-else-if="!orphans.length" class="state" role="status" data-testid="orphans-empty">Старых разговоров нет</p>
    <ul v-else class="list">
      <li v-for="orphan in orphans" :key="orphan.session_id" class="row" data-testid="orphan-row" :data-session-id="orphan.session_id">
        <div class="info">
          <span class="name" :title="orphan.name">{{ orphan.name }}</span>
          <span class="meta">
            <span class="kind">{{ orphan.kind }}</span>
            <span class="branch" :title="orphan.branch">{{ orphan.branch }}</span>
            <time v-if="startedAt(orphan)">{{ startedAt(orphan) }}</time>
          </span>
        </div>
        <VSelect
          class="picker"
          size="small"
          :model-value="targetOf(orphan.session_id)"
          :options="options"
          option-label="label"
          option-value="id"
          aria-label="Worktree для продолжения"
          data-testid="orphan-worktree"
          @update:model-value="pick(orphan.session_id, $event)"
        />
        <VButton
          label="Продолжить"
          size="small"
          data-testid="orphan-resume"
          :loading="resume.busy.value && resuming === orphan.session_id"
          :disabled="!targetOf(orphan.session_id) || resume.busy.value"
          @click="resumeSession(orphan.session_id)"
        />
        <VMessage v-if="resume.error.value && resuming === orphan.session_id" class="failure" severity="error" size="small" data-testid="orphan-error">{{ resume.error.value }}</VMessage>
      </li>
    </ul>
  </VDialog>
</template>

<style scoped>
.state {
  margin: 0;
  color: var(--vr-color-text-muted);
}

.list {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 14rem auto;
  align-items: center;
  gap: var(--vr-space-2) var(--vr-space-3);
  padding: var(--vr-space-2);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
}

.info {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.name,
.branch {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.name {
  font-weight: 600;
}

.meta {
  display: flex;
  gap: var(--vr-space-2);
  min-width: 0;
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
}

.branch {
  font-family: var(--vr-font-mono);
}

.kind,
time {
  flex: none;
}

.picker {
  min-width: 0;
}

.failure {
  grid-column: 1 / -1;
}
</style>

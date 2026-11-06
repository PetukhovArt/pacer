<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue';
import { RefreshCw } from 'lucide-vue-next';
import { VButton, VDialog, VMessage } from '@ui/kit';
import { useI18n } from '../i18n';
import { candidates, cleanupDiff, cleanupOpen, measureSizes, removeCandidates, scanError, scanned, scanNow, scanning, sizes, type Candidate, type RemoveResult } from './state';

const { t, formatRelative, formatBytes, formatNumber } = useI18n();

const selected = shallowRef<ReadonlySet<string>>(new Set());
const decided = new Set<string>();
const deleteBranch = shallowRef(false);
const busy = shallowRef(false);
const progress = shallowRef<string | null>(null);
const results = shallowRef<readonly RemoveResult[]>([]);
const diffs = shallowRef<ReadonlyMap<string, string>>(new Map());

const groups = computed(() => {
  const out = new Map<string, { id: string; name: string; base: string | null; items: Candidate[] }>();
  for (const candidate of candidates.value) {
    const group = out.get(candidate.projectId) ?? { id: candidate.projectId, name: candidate.projectName, base: candidate.base, items: [] };
    group.items.push(candidate);
    out.set(candidate.projectId, group);
  }
  return [...out.values()];
});

const notes = computed(() => {
  const out = new Map<string, string[]>();
  for (const project of scanned.value?.projects ?? []) {
    const list: string[] = [];
    if (project.error) list.push(project.error);
    if (!project.base) list.push(t('cleanup.noBase'));
    else if (!project.fetched) list.push(t('cleanup.noFetch'));
    out.set(project.id, list);
  }
  return out;
});

const picked = computed(() => candidates.value.filter((candidate) => selected.value.has(candidate.key)));
const failures = computed(() => results.value.filter((result) => result.error));
const removedCount = computed(() => results.value.filter((result) => !result.error).length);

watch(
  candidates,
  (list) => {
    const next = new Set<string>();
    for (const candidate of list) {
      const preChecked = candidate.category === 'safe' || candidate.category === 'orphan';
      if (decided.has(candidate.key) ? selected.value.has(candidate.key) : preChecked) next.add(candidate.key);
    }
    selected.value = next;
  },
  { immediate: true },
);

watch(
  cleanupOpen,
  (open) => {
    if (!open) return;
    results.value = [];
    if (!scanned.value) void scanNow();
    else measureSizes();
  },
  { immediate: true },
);

function toggle(key: string, on: boolean) {
  decided.add(key);
  const next = new Set(selected.value);
  if (on) next.add(key);
  else next.delete(key);
  selected.value = next;
}

function requestLabel(candidate: Candidate): string {
  const request = candidate.scan?.request;
  if (!request) return '';
  return request.kind === 'gitlab' ? `MR !${request.number}` : `PR #${request.number}`;
}

function reasonOf(candidate: Candidate): string {
  if (candidate.orphan) return t(`cleanup.reason.${candidate.orphan.kind}`);
  const scan = candidate.scan;
  if (!scan) return '';
  switch (scan.reason) {
    case 'request_merged':
    case 'request_closed':
      return t(`cleanup.reason.${scan.reason}`, { request: requestLabel(candidate) });
    case 'merged':
      return t('cleanup.reason.merged', { base: candidate.base ?? '' });
    case 'stale':
      return t('cleanup.reason.stale', { days: scan.idleDays ?? 0 });
    default:
      return t(`cleanup.reason.${scan.reason}`);
  }
}

function detailsOf(candidate: Candidate): string[] {
  const scan = candidate.scan;
  if (!scan) return [];
  const out: string[] = [];
  if (scan.live) out.push(t('cleanup.live'));
  if (scan.changes) out.push(t('cleanup.changesCount', { count: scan.changes }));
  if (scan.untracked) out.push(t('cleanup.untrackedCount', { count: scan.untracked }));
  if (scan.inProgress) out.push(t('cleanup.inProgress'));
  return out;
}

function sizeOf(candidate: Candidate): string {
  if (candidate.scan?.reason === 'missing') return '';
  const size = sizes.value.get(candidate.path);
  if (size === undefined) return t('cleanup.sizing');
  return size < 1024 * 1024 ? formatNumber(Math.max(1, Math.ceil(size / 1024)), { style: 'unit', unit: 'kilobyte', unitDisplay: 'short' }) : formatBytes(size);
}

async function toggleDiff(candidate: Candidate) {
  const next = new Map(diffs.value);
  if (next.has(candidate.key)) {
    next.delete(candidate.key);
    diffs.value = next;
    return;
  }
  try {
    next.set(candidate.key, await cleanupDiff(candidate.path));
  } catch (error) {
    next.set(candidate.key, error instanceof Error ? error.message : String(error));
  }
  diffs.value = new Map(next);
}

function lineKind(line: string): string {
  if (line.startsWith('+') && !line.startsWith('+++')) return 'add';
  if (line.startsWith('-') && !line.startsWith('---')) return 'del';
  if (line.startsWith('@@')) return 'hunk';
  return '';
}

async function removeSelected() {
  const list = picked.value;
  if (!list.length || busy.value) return;
  busy.value = true;
  results.value = [];
  try {
    results.value = await removeCandidates(list, deleteBranch.value, (name) => (progress.value = name));
  } finally {
    busy.value = false;
    progress.value = null;
  }
}
</script>

<template>
  <VDialog
    :visible="cleanupOpen"
    modal
    dismissable-mask
    :header="t('cleanup.title')"
    :style="{ width: '56rem', maxWidth: '95vw' }"
    data-testid="cleanup-dialog"
    @update:visible="cleanupOpen = $event"
  >
    <div class="top">
      <p class="intro">{{ t('cleanup.intro') }}</p>
      <span class="when">
        <span v-if="scanned">{{ t('cleanup.scannedAt', { when: formatRelative(scanned.at) }) }}</span>
        <button class="icon" :title="t('cleanup.rescan')" :aria-label="t('cleanup.rescan')" :disabled="scanning || busy" data-testid="cleanup-rescan" @click="scanNow"><RefreshCw :size="14" :class="{ spin: scanning }" /></button>
      </span>
    </div>
    <VMessage v-if="scanError" severity="error" size="small" data-testid="cleanup-error">{{ scanError }}</VMessage>
    <p v-if="scanning && !candidates.length" class="state" role="status">{{ t('cleanup.scanning') }}</p>
    <p v-else-if="!candidates.length" class="state" role="status" data-testid="cleanup-empty">{{ t('cleanup.empty') }}</p>
    <section v-for="group in groups" :key="group.id" class="group" data-testid="cleanup-group" :data-project="group.name">
      <h3 class="project">
        {{ group.name }}
        <span v-for="note in notes.get(group.id) ?? []" :key="note" class="note">{{ note }}</span>
      </h3>
      <ul class="list">
        <li v-for="candidate in group.items" :key="candidate.key" class="row" data-testid="cleanup-row" :data-category="candidate.category" :data-path="candidate.path" :data-name="candidate.name">
          <label class="main">
            <input
              type="checkbox"
              class="check"
              :checked="selected.has(candidate.key)"
              :disabled="busy"
              :aria-label="candidate.name"
              data-testid="cleanup-check"
              @change="toggle(candidate.key, ($event.target as HTMLInputElement).checked)"
            />
            <span class="name" :title="candidate.path">{{ candidate.name }}</span>
            <span class="chip" :data-category="candidate.category">{{ t(`cleanup.category.${candidate.category}`) }}</span>
            <span class="reason">{{ reasonOf(candidate) }}</span>
            <span v-for="detail in detailsOf(candidate)" :key="detail" class="detail">{{ detail }}</span>
            <span class="size" data-testid="cleanup-size">{{ sizeOf(candidate) }}</span>
          </label>
          <button v-if="candidate.category === 'changes'" class="link" data-testid="cleanup-diff" @click="toggleDiff(candidate)">{{ diffs.has(candidate.key) ? t('cleanup.hideDiff') : t('cleanup.showDiff') }}</button>
          <pre v-if="diffs.has(candidate.key)" class="diff" data-testid="cleanup-diff-text"><span v-for="(line, index) in (diffs.get(candidate.key) ?? '').split('\n')" :key="index" :data-kind="lineKind(line)">{{ line }}
</span></pre>
        </li>
      </ul>
    </section>
    <VMessage v-if="removedCount && !busy" severity="success" size="small" data-testid="cleanup-removed">{{ t('cleanup.removed', { count: removedCount }) }}</VMessage>
    <VMessage v-for="failure in failures" :key="failure.key" severity="error" size="small" data-testid="cleanup-failure">{{ t('cleanup.failed', { name: failure.name, message: failure.error ?? '' }) }}</VMessage>
    <template #footer>
      <label class="branch">
        <input v-model="deleteBranch" type="checkbox" class="check" :disabled="busy" data-testid="cleanup-delete-branch" />
        {{ t('cleanup.deleteBranch') }}
      </label>
      <span class="progress" role="status">{{ progress ? t('cleanup.removing', { name: progress }) : t('cleanup.selected', { count: picked.length }) }}</span>
      <VButton :label="t('cleanup.removeSelected')" severity="danger" size="small" :loading="busy" :disabled="!picked.length || busy || scanning" data-testid="cleanup-remove" @click="removeSelected" />
    </template>
  </VDialog>
</template>

<style scoped>
.top {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--vr-space-3);
  margin-bottom: var(--vr-space-2);
}

.intro {
  margin: 0;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-muted);
}

.when {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-1);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
}

.icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.icon:hover:not(:disabled) {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.state {
  margin: 0;
  color: var(--vr-color-text-muted);
}

.group + .group {
  margin-top: var(--vr-space-3);
}

.project {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--vr-space-2);
  margin: 0 0 var(--vr-space-1);
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
}

.note {
  font-size: var(--vr-font-size-2xs);
  font-weight: 400;
  color: var(--vr-color-text-muted);
}

.list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.row {
  padding: var(--vr-space-1) var(--vr-space-2);
  border-radius: var(--vr-radius-sm);
}

.row:hover {
  background: var(--vr-color-nav-hover);
}

.main {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
  cursor: pointer;
}

.check {
  flex: none;
  margin: 0;
  accent-color: var(--vr-color-accent);
}

.name {
  min-width: 0;
  overflow: hidden;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.chip {
  flex: none;
  padding: 0 var(--vr-space-1);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.chip[data-category='safe'] {
  color: var(--vr-color-success);
}

.chip[data-category='changes'] {
  color: var(--vr-color-conflict);
}

.chip[data-category='orphan'] {
  color: var(--vr-color-accent);
}

.reason,
.detail {
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
}

.detail {
  color: var(--vr-color-conflict);
}

.size {
  margin-left: auto;
  font-size: var(--vr-font-size-2xs);
  font-variant-numeric: tabular-nums;
  color: var(--vr-color-text-muted);
}

.link {
  margin: 2px 0 0 calc(13px + var(--vr-space-2));
  padding: 0;
  font: inherit;
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-accent);
  background: none;
  border: 0;
  cursor: pointer;
}

.diff {
  max-height: 18rem;
  margin: var(--vr-space-1) 0 0;
  padding: var(--vr-space-2);
  overflow: auto;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  white-space: pre;
  background: var(--vr-color-surface-muted);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-sm);
}

.diff [data-kind='add'] {
  color: var(--vr-color-success);
}

.diff [data-kind='del'] {
  color: var(--vr-color-danger);
}

.diff [data-kind='hunk'] {
  color: var(--vr-color-accent);
}

.branch {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  margin-right: auto;
  font-size: var(--vr-font-size-xs);
  cursor: pointer;
}

.progress {
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
}
</style>

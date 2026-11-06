<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, shallowRef, useTemplateRef, watch } from 'vue';
import { Check, Copy, FolderOpen, GitBranch, Play, SquareTerminal, TriangleAlert, Wrench } from 'lucide-vue-next';
import { VButton, VMessage } from '@ui/kit';
import { copyText } from '../atlassian/util';
import RichContent from '../content/RichContent.vue';
import { continueInPacer, externalDot, externalKey, externalOrigin, externalStatusLabel, externalTarget, readTranscript, resumeCommand, useExternalSessions, type Harness, type TranscriptMessage } from '../agents/external';
import HarnessIcon from '../agents/HarnessIcon.vue';
import { describeError, usePacer } from '../agents/pacer';
import StatusDot from '../agents/StatusDot.vue';
import { useAction } from '../agents/useAction';
import { useI18n } from '../i18n';
import { useDock } from './dock';

const TAIL = 200;
const PIN_PX = 48;
const COPIED_MS = 1500;

const props = defineProps<{ harness: Harness; sessionId: string }>();

const { t, formatRelative, formatDate } = useI18n();
const external = useExternalSessions();
const pacer = usePacer();
const dock = useDock();
const action = useAction();
const scroller = useTemplateRef<HTMLElement>('scroller');
const messages = shallowRef<TranscriptMessage[]>([]);
const loading = shallowRef(true);
const error = shallowRef<string | null>(null);
const copied = shallowRef(false);
let generation = 0;
let copiedTimer: ReturnType<typeof setTimeout> | undefined;

const session = computed(() => external?.anyByKey.value.get(externalKey({ harness: props.harness, id: props.sessionId })));
const target = computed(() => (session.value ? externalTarget(session.value, pacer?.entities.value) : null));
const running = computed(() => session.value?.running ?? false);
const command = computed(() => resumeCommand({ harness: props.harness, id: props.sessionId }));
const canContinue = computed(() => !!session.value && !!target.value && !running.value && !action.busy.value);

async function load() {
  const current = ++generation;
  const el = scroller.value;
  const pinned = !el || el.scrollHeight - el.scrollTop - el.clientHeight < PIN_PX;
  try {
    const next = await readTranscript({ harness: props.harness, id: props.sessionId }, TAIL);
    if (current !== generation) return;
    messages.value = next;
    error.value = null;
  } catch (e) {
    if (current === generation) error.value = describeError(e);
  } finally {
    if (current === generation) loading.value = false;
  }
  if (!pinned || current !== generation) return;
  await nextTick();
  const box = scroller.value;
  if (box) box.scrollTop = box.scrollHeight;
}

const root = useTemplateRef<HTMLElement>('root');
let loaded = false;
let shown = true;
let stale = false;
let observer: IntersectionObserver | undefined;

watch(
  () => [props.harness, props.sessionId, session.value?.updated_at] as const,
  () => {
    if (loaded && !shown) {
      stale = true;
      return;
    }
    loaded = true;
    void load();
  },
  { immediate: true },
);

onMounted(() => {
  const element = root.value;
  if (!element) return;
  observer = new IntersectionObserver((entries) => {
    const latest = entries.at(-1);
    if (!latest || latest.isIntersecting === shown) return;
    shown = latest.isIntersecting;
    if (shown && stale) {
      stale = false;
      void load();
    }
  });
  observer.observe(element);
});

async function copyCommand() {
  await copyText(command.value);
  copied.value = true;
  clearTimeout(copiedTimer);
  copiedTimer = setTimeout(() => (copied.value = false), COPIED_MS);
}

async function continueHere() {
  const current = session.value;
  const where = target.value;
  if (!current || !where || current.running) return;
  await action.run(async () => {
    const ref = await continueInPacer(current, where);
    dock.openWhenListed(ref);
  });
}

onBeforeUnmount(() => {
  clearTimeout(copiedTimer);
  observer?.disconnect();
});
</script>

<template>
  <section ref="root" class="conversation" data-testid="conversation-view" :data-session-id="sessionId" :data-harness="harness">
    <header class="head">
      <div class="title-line">
        <HarnessIcon :harness="harness" :size="14" />
        <h2 class="title">{{ session?.title ?? sessionId }}</h2>
        <span v-if="session" class="status" data-testid="conversation-status" :data-running="session.running" :data-status="session.status">
          <StatusDot :status="externalDot(session)" :unseen="false" />
          {{ externalStatusLabel(session) }}
        </span>
      </div>
      <div class="meta">
        <span class="chip">{{ t(`workbench.external.harness.${harness}`) }}</span>
        <span v-if="session" class="chip mono" :title="session.cwd"><FolderOpen :size="12" />{{ session.cwd }}</span>
        <span v-if="session?.branch" class="chip mono"><GitBranch :size="12" />{{ session.branch }}</span>
        <span v-if="session" class="chip" :title="formatDate(session.updated_at)">{{ formatRelative(session.updated_at) }}</span>
        <span v-if="session" class="chip" data-testid="conversation-origin"><SquareTerminal :size="12" />{{ externalOrigin(session) }}</span>
      </div>
      <div class="actions">
        <VButton size="small" :disabled="!canContinue" :loading="action.busy.value" data-testid="conversation-continue" @click="continueHere">
          <Play :size="13" />
          {{ t('workbench.external.continue') }}
        </VButton>
        <VButton size="small" severity="secondary" outlined :title="command" data-testid="conversation-copy" @click="copyCommand">
          <Check v-if="copied" :size="13" />
          <Copy v-else :size="13" />
          {{ copied ? t('workbench.external.copied') : t('workbench.external.copyResume') }}
        </VButton>
        <code class="command mono">{{ command }}</code>
      </div>
      <p v-if="running && session?.in_pacer" class="hint" data-testid="conversation-in-terminal">{{ t('workbench.external.liveInTerminal') }}</p>
      <p v-else-if="running" class="warning" role="alert" data-testid="conversation-running-warning"><TriangleAlert :size="13" />{{ t('workbench.external.stillOpen') }}</p>
      <p v-else-if="session && !target" class="hint" data-testid="conversation-no-project">{{ t('workbench.external.noProject') }}</p>
      <p v-else-if="target" class="hint" data-testid="conversation-target">{{ t('workbench.external.continuesIn', { project: target.projectName, branch: target.branch }) }}</p>
      <VMessage v-if="action.error.value" severity="error" size="small" data-testid="conversation-error">{{ action.error.value }}</VMessage>
    </header>
    <div ref="scroller" class="scroll" data-testid="conversation-scroll">
      <p v-if="loading" class="state">{{ t('workbench.external.loading') }}</p>
      <p v-else-if="error" class="state error" role="alert">{{ error }}</p>
      <p v-else-if="!messages.length" class="state">{{ t('workbench.external.empty') }}</p>
      <ol v-else class="messages" data-testid="conversation-messages">
        <li v-for="message in messages" :key="message.key" class="message" :data-role="message.role" :data-tool="message.tool !== null" :title="message.at ? formatDate(message.at) : undefined" data-testid="conversation-message">
          <div v-if="message.tool" class="tool">
            <Wrench :size="12" />
            <span class="tool-name">{{ message.tool }}</span>
            <span class="tool-arg mono">{{ message.text }}</span>
          </div>
          <p v-else-if="message.role === 'user'" class="user">{{ message.text }}</p>
          <RichContent v-else class="assistant" :markdown="message.text" />
        </li>
      </ol>
    </div>
  </section>
</template>

<style scoped>
.conversation {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background: var(--vr-color-surface);
}

.head {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: var(--vr-space-2);
  padding: var(--vr-space-3) var(--vr-space-4);
  border-bottom: 1px solid var(--vr-color-border);
}

.title-line {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
}

.title {
  flex: 1;
  min-width: 0;
  margin: 0;
  overflow: hidden;
  font-size: var(--vr-font-size-base);
  font-weight: 600;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.status {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-1);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.meta,
.actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-1) var(--vr-space-2);
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  max-width: 100%;
  padding: 0 var(--vr-space-2);
  overflow: hidden;
  font-size: var(--vr-font-size-2xs);
  line-height: 20px;
  color: var(--vr-color-text-muted);
  white-space: nowrap;
  text-overflow: ellipsis;
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.mono {
  font-family: var(--vr-font-mono);
}

.command {
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
  user-select: all;
}

.warning,
.hint {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  margin: 0;
  font-size: var(--vr-font-size-xs);
}

.warning {
  color: var(--vr-color-danger);
}

.hint {
  color: var(--vr-color-text-muted);
}

.scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--vr-space-3) var(--vr-space-4) var(--vr-space-6);
}

.state {
  margin: 0;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-muted);
}

.state.error {
  color: var(--vr-color-danger);
}

.messages {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  max-width: 920px;
  margin: 0 auto;
  padding: 0;
  list-style: none;
}

.message[data-role='user'] {
  align-self: flex-end;
  max-width: 85%;
}

.user {
  margin: 0;
  padding: var(--vr-space-2) var(--vr-space-3);
  font-size: var(--vr-font-size-sm);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-lg);
}

.assistant {
  font-size: var(--vr-font-size-sm);
}

.message[data-tool='true'] + .message[data-tool='true'] {
  margin-top: calc(-1 * var(--vr-space-2));
}

.tool {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
  padding: 2px var(--vr-space-2);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
}

.tool-name {
  flex: none;
  font-weight: 600;
}

.tool-arg {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
</style>

<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import DOMPurify from 'dompurify';
import { marked } from 'marked';
import { GitMerge, Wrench } from 'lucide-vue-next';
import type { VeyraEvent } from '@veyra/protocol';
import Avatar from '../Avatar.vue';
import type { TaskContext } from './task';

const props = defineProps<{ ctx: TaskContext }>();
const { client, events, agentOf } = props.ctx;

type ToolCall = Extract<VeyraEvent, { type: 'agent.tool_call' }>;
type Item = { id: string; actorId: string; at: string; head: boolean } & ({ kind: 'event'; event: VeyraEvent } | { kind: 'tools'; calls: ToolCall[] });

const GROUP_MS = 5 * 60_000;

const items = computed(() =>
  events.value.reduce<Item[]>((list, event) => {
    if (event.type === 'agent.todos') return list;
    const last = list[list.length - 1];
    if (event.type === 'agent.tool_call' && last?.kind === 'tools' && last.actorId === event.actorId) {
      last.calls.push(event);
      return list;
    }
    const head = !last || last.actorId !== event.actorId || new Date(event.at).getTime() - new Date(last.at).getTime() > GROUP_MS;
    const base = { id: event.id, actorId: event.actorId, at: event.at, head };
    list.push(event.type === 'agent.tool_call' ? { ...base, kind: 'tools', calls: [event] } : { ...base, kind: 'event', event });
    return list;
  }, []),
);

function markdown(text: string): string {
  return DOMPurify.sanitize(marked.parse(text, { async: false, gfm: true, breaks: true }));
}

function time(at: string) {
  return new Date(at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
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
      return `закрепил [${event.pin.kind}]: ${event.pin.text}`;
    case 'task.assigned':
      return 'взял задачу';
    case 'agent.started':
      return 'запустил агента';
    case 'task.updated':
      return `трекер: ${event.title} (${event.status})`;
    case 'task.mr':
      return `MR ${event.state}`;
    case 'chat.message':
      return event.message.text;
    case 'agent.reply':
      return event.text;
    case 'agent.session':
      return `сессия ${event.sessionId}`;
    case 'agent.step':
      return `jig · ${event.unit}: ${event.status}`;
    case 'agent.todos':
      return `план: ${event.items.filter((i) => i.status === 'completed').length}/${event.items.length}`;
  }
}

function isSystem(event: VeyraEvent) {
  return !['chat.message', 'agent.message', 'agent.reply', 'agent.started', 'task.mr'].includes(event.type);
}

const scroller = ref<HTMLElement | null>(null);

function toBottom() {
  const el = scroller.value;
  if (el) el.scrollTop = el.scrollHeight;
}

watch(
  () => items.value.length,
  async () => {
    const el = scroller.value;
    const near = !el || el.scrollHeight - el.scrollTop - el.clientHeight < 120;
    await nextTick();
    if (near) toBottom();
  },
);
onMounted(toBottom);
</script>

<template>
  <div ref="scroller" class="scroller">
    <slot />
    <p v-if="!items.length" class="vr-muted intro">Здесь появятся сообщения команды и ход работы агентов.</p>
    <ol class="feed" data-testid="feed">
      <li v-for="item in items" :key="item.id" class="row" :class="{ head: item.head }" :data-kind="item.kind === 'event' ? item.event.type : 'tools'">
        <Avatar v-if="item.head" :id="item.actorId" :name="client.actorName(item.actorId)" :agent="!!agentOf(item.actorId)" />
        <span v-else class="gutter">{{ time(item.at) }}</span>
        <div class="body">
          <div v-if="item.head" class="author">
            <strong>{{ client.actorName(item.actorId) }}</strong>
            <time :datetime="item.at">{{ time(item.at) }}</time>
          </div>
          <details v-if="item.kind === 'tools'" class="tools">
            <summary><Wrench :size="12" /> инструменты: {{ item.calls.length }} <span class="last">{{ item.calls[item.calls.length - 1]!.tool }} {{ item.calls[item.calls.length - 1]!.summary }}</span></summary>
            <div v-for="call in item.calls" :key="call.id" class="tool-call">
              <code class="tool-name">{{ call.tool }}</code>
              <span class="tool-summary" :title="call.summary">{{ call.summary }}</span>
            </div>
          </details>
          <div v-else-if="item.event.type === 'agent.message'" class="markdown" v-html="markdown(item.event.text)" />
          <p v-else-if="item.event.type === 'agent.reply'" class="text">
            <span class="vr-chip">@{{ client.actorName(item.event.agentId) }}</span> {{ item.event.text }}
          </p>
          <p v-else-if="item.event.type === 'agent.started'" class="text">
            <span class="vr-chip">запуск агента</span> {{ item.event.prompt }}
          </p>
          <template v-else-if="item.event.type === 'task.mr'">
            <p class="text system">{{ describe(item.event) }}</p>
            <a class="link-card" :href="item.event.url" target="_blank" rel="noopener">
              <span class="link-icon"><GitMerge :size="16" /></span>
              <span class="link-text">
                <span class="vr-muted">GitLab · MR</span>
                <strong>{{ item.event.title }}</strong>
              </span>
            </a>
          </template>
          <p v-else class="text" :class="{ system: isSystem(item.event) }">{{ describe(item.event) }}</p>
        </div>
      </li>
    </ol>
  </div>
</template>

<style scoped>
.scroller {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--vr-space-3) var(--vr-space-3) var(--vr-space-2);
}

.intro {
  padding: 0 var(--vr-space-2);
}

.feed {
  margin: 0;
  padding: 0;
  list-style: none;
}

.row {
  display: grid;
  grid-template-columns: var(--vr-size-avatar) minmax(0, 1fr);
  gap: var(--vr-space-3);
  padding: var(--vr-space-1) var(--vr-space-2);
  border-radius: var(--vr-radius-2xl);
}

.row.head {
  margin-top: var(--vr-space-2);
}

.row:hover {
  background: color-mix(in srgb, var(--vr-color-surface-muted) 50%, transparent);
}

.gutter {
  visibility: hidden;
  padding-top: 2px;
  font-size: var(--vr-font-size-2xs);
  text-align: right;
  color: var(--vr-color-text-muted);
}

.row:hover .gutter {
  visibility: visible;
}

.author {
  display: flex;
  align-items: baseline;
  gap: var(--vr-space-2);
  line-height: var(--vr-font-leading-author);
  margin-bottom: 2px;
}

time {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.text {
  margin: 0;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.text.system {
  color: var(--vr-color-text-muted);
}

.link-card {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-3);
  max-width: 360px;
  margin-top: var(--vr-space-2);
  padding: var(--vr-space-2) var(--vr-space-3);
  color: inherit;
  text-decoration: none;
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-xl);
}

.link-card:hover {
  background: var(--vr-color-surface-muted);
}

.link-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  flex: none;
  border-radius: var(--vr-radius-md);
  background: var(--vr-color-surface-muted);
}

.link-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
  font-size: var(--vr-font-size-xs);
  line-height: 16px;
}

.link-text strong {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.tools summary {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  min-width: 0;
  color: var(--vr-color-text-muted);
  list-style: none;
}

.tools .last {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
}

.tool-call {
  display: flex;
  gap: var(--vr-space-2);
  min-width: 0;
  padding-left: var(--vr-space-4);
}

.tool-name {
  flex: none;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  font-weight: 600;
}

.tool-summary {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
}

.markdown {
  overflow-wrap: anywhere;
}

.markdown :deep(p),
.markdown :deep(ul),
.markdown :deep(ol),
.markdown :deep(pre) {
  margin: 0 0 var(--vr-space-2);
}

.markdown :deep(> :last-child) {
  margin-bottom: 0;
}

.markdown :deep(ul),
.markdown :deep(ol) {
  padding-left: var(--vr-space-5);
}

.markdown :deep(:not(pre) > code) {
  padding: 1px 4px;
  font-family: var(--vr-font-mono);
  font-size: 0.9em;
  border-radius: var(--vr-radius-sm);
  background: var(--vr-color-surface-muted);
}

.markdown :deep(pre) {
  overflow: auto;
  padding: var(--vr-space-3);
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-lg);
}
</style>

<script setup lang="ts" generic="T">
import { computed, nextTick, shallowRef, useId, useTemplateRef, watch } from 'vue';
import { ChevronRight } from 'lucide-vue-next';
import type { TreeNode, TreeRow } from './types';

const props = withDefaults(
  defineProps<{
    nodes: TreeNode<T>[];
    storageKey: string;
    label: string;
    selected?: string | null;
    revealAll?: boolean;
    testId?: string;
  }>(),
  { selected: null, revealAll: false, testId: 'explorer-tree' },
);

const emit = defineEmits<{ open: [node: TreeNode<T>]; toggle: [id: string, expanded: boolean] }>();

defineSlots<{
  label?(props: { node: TreeNode<T>; expanded: boolean }): unknown;
  leaf?(props: { node: TreeNode<T>; data: T | undefined; selected: boolean }): unknown;
  rollup?(props: { node: TreeNode<T> }): unknown;
  actions?(props: { node: TreeNode<T>; row: TreeRow<T> }): unknown;
  empty?(): unknown;
}>();

const TYPEAHEAD_MS = 600;

const uid = useId();
const tree = useTemplateRef<HTMLElement>('tree');
const cursor = shallowRef<string | null>(null);

function readOverrides(key: string): Record<string, boolean> {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(key) ?? 'null');
    if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return {};
    return Object.fromEntries(Object.entries(raw).filter((entry): entry is [string, boolean] => typeof entry[1] === 'boolean'));
  } catch {
    return {};
  }
}

const overrides = shallowRef<Readonly<Record<string, boolean>>>(readOverrides(props.storageKey));
const revealCollapsed = shallowRef<ReadonlySet<string>>(new Set());

watch(
  () => props.storageKey,
  (key) => (overrides.value = readOverrides(key)),
);
watch(
  () => props.revealAll,
  () => (revealCollapsed.value = new Set()),
);

function isExpanded(node: TreeNode<T>): boolean {
  if (props.revealAll) return !revealCollapsed.value.has(node.id);
  return overrides.value[node.id] ?? node.defaultExpanded ?? true;
}

function setExpanded(id: string, want: boolean) {
  if (props.revealAll) {
    const next = new Set(revealCollapsed.value);
    if (want) next.delete(id);
    else next.add(id);
    revealCollapsed.value = next;
  } else {
    overrides.value = { ...overrides.value, [id]: want };
    localStorage.setItem(props.storageKey, JSON.stringify(overrides.value));
  }
  emit('toggle', id, want);
}

const rows = computed<TreeRow<T>[]>(() => {
  const out: TreeRow<T>[] = [];
  const walk = (nodes: TreeNode<T>[], depth: number, parent: string | null) => {
    for (const node of nodes) {
      const group = node.children !== undefined;
      const expanded = group && isExpanded(node);
      out.push({ id: node.id, depth, parent, node, group, expanded });
      if (expanded && node.children) walk(node.children, depth + 1, node.id);
    }
  };
  walk(props.nodes, 0, null);
  return out;
});

const index = computed(() => {
  const map = new Map<string, { parent: string | null; node: TreeNode<T> }>();
  const walk = (nodes: TreeNode<T>[], parent: string | null) => {
    for (const node of nodes) {
      map.set(node.id, { parent, node });
      if (node.children) walk(node.children, node.id);
    }
  };
  walk(props.nodes, null);
  return map;
});

const domId = (id: string) => `${uid}-${id.replace(/\s+/g, '_')}`;
const activeDescendant = computed(() => (cursor.value && rows.value.some((row) => row.id === cursor.value) ? domId(cursor.value) : undefined));

function setCursor(id: string) {
  cursor.value = id;
  void nextTick(() => {
    const target = Array.from(tree.value?.querySelectorAll<HTMLElement>('[data-row-id]') ?? []).find((element) => element.dataset.rowId === id);
    target?.scrollIntoView({ block: 'nearest' });
  });
}

const revealed = new Set<string>();

watch(
  () => [props.selected, index.value] as const,
  ([id]) => {
    if (!id) revealed.clear();
    if (!id || revealed.has(id) || !index.value.has(id)) return;
    revealed.clear();
    revealed.add(id);
    const shut: string[] = [];
    for (let parent = index.value.get(id)?.parent ?? null; parent; parent = index.value.get(parent)?.parent ?? null) {
      const entry = index.value.get(parent);
      if (entry && !isExpanded(entry.node)) shut.push(parent);
    }
    if (shut.length && !props.revealAll) {
      overrides.value = { ...overrides.value, ...Object.fromEntries(shut.map((parent) => [parent, true])) };
      localStorage.setItem(props.storageKey, JSON.stringify(overrides.value));
    }
    setCursor(id);
  },
  { immediate: true },
);

function focusTree() {
  tree.value?.focus({ preventScroll: true });
}

function onRowClick(row: TreeRow<T>) {
  setCursor(row.id);
  if (row.group) setExpanded(row.id, !row.expanded);
  else emit('open', row.node);
  focusTree();
}

function onFocus() {
  if (cursor.value && rows.value.some((row) => row.id === cursor.value)) return;
  const first = rows.value.find((row) => row.id === props.selected) ?? rows.value[0];
  if (first) cursor.value = first.id;
}

let typed = '';
let typedAt = 0;

function typeAhead(char: string, from: number): boolean {
  const now = Date.now();
  typed = now - typedAt > TYPEAHEAD_MS ? char : typed + char;
  typedAt = now;
  const list = rows.value;
  const needle = typed.toLowerCase();
  const start = typed.length > 1 ? from : from + 1;
  for (let step = 0; step < list.length; step++) {
    const row = list[(Math.max(0, start) + step) % list.length];
    if (row?.node.label.toLowerCase().startsWith(needle)) {
      setCursor(row.id);
      return true;
    }
  }
  return false;
}

function onKey(event: KeyboardEvent) {
  if (event.target !== tree.value) return;
  const list = rows.value;
  const index = list.findIndex((row) => row.id === cursor.value);
  const current = index >= 0 ? list[index] : undefined;
  const move = (to: number) => {
    const row = list[Math.max(0, Math.min(list.length - 1, to))];
    if (row) setCursor(row.id);
  };
  switch (event.key) {
    case 'ArrowDown':
      move(index + 1);
      break;
    case 'ArrowUp':
      move(index < 0 ? 0 : index - 1);
      break;
    case 'Home':
      move(0);
      break;
    case 'End':
      move(list.length - 1);
      break;
    case 'ArrowRight':
      if (!current?.group) return;
      if (!current.expanded) setExpanded(current.id, true);
      else move(index + 1);
      break;
    case 'ArrowLeft':
      if (!current) return;
      if (current.group && current.expanded) setExpanded(current.id, false);
      else if (current.parent) setCursor(current.parent);
      break;
    case 'Enter':
    case ' ':
      if (!current) return;
      if (current.group) setExpanded(current.id, !current.expanded);
      else emit('open', current.node);
      break;
    default:
      if (event.key.length !== 1 || event.ctrlKey || event.metaKey || event.altKey || !typeAhead(event.key, index)) return;
  }
  event.preventDefault();
}
</script>

<template>
  <div
    ref="tree"
    v-auto-animate
    class="tree"
    role="tree"
    tabindex="0"
    :aria-label="label"
    :aria-activedescendant="activeDescendant"
    :data-testid="testId"
    @keydown="onKey"
    @focus="onFocus"
  >
    <div
      v-for="row in rows"
      :id="domId(row.id)"
      :key="row.id"
      class="node"
      :class="row.group ? 'group' : 'leaf'"
      role="treeitem"
      :aria-level="row.depth + 1"
      :aria-expanded="row.group ? row.expanded : undefined"
      :aria-selected="row.id === selected"
      :data-row-id="row.id"
      :data-kind="row.node.kind"
      :data-cursor="row.id === cursor"
      :data-selected="row.id === selected"
      :style="{ '--depth': row.depth }"
    >
      <i v-for="level in row.depth" :key="level" class="guide" :style="{ '--level': level - 1 }" />
      <button class="toggle" tabindex="-1" :title="row.node.title ?? row.node.label" :data-testid="`tree-${row.node.kind}`" @click="onRowClick(row)">
        <ChevronRight v-if="row.group" :size="14" class="chevron vr-chevron" :data-open="row.expanded" />
        <component :is="row.expanded && row.node.iconOpen ? row.node.iconOpen : row.node.icon" v-if="row.node.icon" :size="14" class="kind-icon" />
        <template v-if="row.group">
          <slot name="label" :node="row.node" :expanded="row.expanded">
            <span class="label" :data-kind="row.node.kind">{{ row.node.label }}</span>
          </slot>
          <template v-if="!row.expanded">
            <span v-if="row.node.count !== undefined" class="count" :title="row.node.countTitle" data-testid="tree-count">{{ row.node.count }}</span>
            <slot name="rollup" :node="row.node" />
          </template>
        </template>
        <slot v-else name="leaf" :node="row.node" :data="row.node.data" :selected="row.id === selected">
          <span class="label" :data-kind="row.node.kind">{{ row.node.label }}</span>
        </slot>
      </button>
      <span v-if="$slots.actions" class="node-actions"><slot name="actions" :node="row.node" :row="row" /></span>
    </div>
    <slot v-if="!rows.length" name="empty" />
  </div>
</template>

<style scoped>
.tree {
  --indent: 14px;
  outline: none;
}

.node {
  position: relative;
  display: flex;
  align-items: center;
  border-radius: var(--vr-radius-md);
}

.node[data-cursor='true'] {
  background: var(--vr-color-nav-hover);
}

.node[data-selected='true'] {
  background: var(--vr-color-nav-active);
}

.tree:focus-visible .node[data-cursor='true'] {
  box-shadow: inset 0 0 0 1px var(--vr-color-accent);
}

.guide {
  position: absolute;
  top: 0;
  bottom: 0;
  left: calc(var(--level) * var(--indent) + 13px);
  width: 1px;
  background: var(--vr-color-border);
  pointer-events: none;
}

.toggle {
  display: flex;
  flex: 1;
  align-items: center;
  gap: var(--vr-space-1);
  min-width: 0;
  min-height: var(--vr-size-row);
  padding: 0 var(--vr-space-1) 0 calc(var(--depth) * var(--indent) + 6px);
  font: inherit;
  text-align: left;
  color: var(--vr-color-text);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
  cursor: pointer;
}

.leaf .toggle {
  padding-left: calc(var(--depth) * var(--indent) + 6px + 14px + var(--vr-space-1));
}

.toggle:hover {
  background: var(--vr-color-nav-hover);
}

.chevron,
.kind-icon {
  flex: none;
  color: var(--vr-color-text-muted);
}

.label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.group > .toggle > .label {
  font-weight: 500;
}

.group[aria-level='1'] > .toggle > .label {
  font-weight: 600;
}

.count {
  flex: none;
  padding: 0 var(--vr-space-2);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.node-actions {
  display: inline-flex;
  flex: none;
  align-items: center;
  opacity: 0;
}

.node:hover .node-actions,
.node[data-cursor='true'] .node-actions,
.node-actions:focus-within {
  opacity: 1;
}
</style>

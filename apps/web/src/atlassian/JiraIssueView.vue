<script setup lang="ts">
import { ref, watch } from 'vue';
import { VButton } from '@ui/kit';
import RichContent from '../content/RichContent.vue';
import { t, useI18n } from '../i18n';
import { describeError, jiraIssue, openUrl, type JiraIssue } from '../connectors/api';
import ErrorInline from './ErrorInline.vue';
import PersonAvatar from './PersonAvatar.vue';
import PriorityMark from './PriorityMark.vue';
import Skeleton from './Skeleton.vue';
import StatusChip from './StatusChip.vue';
import TypeIcon from './TypeIcon.vue';
import { copyText } from './util';

const { formatRelative, formatDate } = useI18n();

const props = defineProps<{ connector: string; issueKey: string }>();
const emit = defineEmits<{ open: [payload: { connector: string; key: string }] }>();

const issue = ref<JiraIssue | null>(null);
const loading = ref(false);
const error = ref('');
const copied = ref(false);
let seq = 0;

async function load() {
  const id = ++seq;
  loading.value = true;
  error.value = '';
  issue.value = null;
  try {
    const result = await jiraIssue(props.connector, props.issueKey);
    if (id === seq) issue.value = result;
  } catch (e) {
    if (id === seq) {
      issue.value = null;
      error.value = describeError(e);
    }
  } finally {
    if (id === seq) loading.value = false;
  }
}

async function copyKey() {
  await copyText(props.issueKey);
  copied.value = true;
  setTimeout(() => (copied.value = false), 1500);
}

function openKey(key: string) {
  emit('open', { connector: props.connector, key });
}

watch(() => [props.connector, props.issueKey], load, { immediate: true });
</script>

<template>
  <div class="issue" data-testid="jira-issue">
    <ErrorInline v-if="error" :message="error" @retry="load" />
    <Skeleton v-else-if="loading && !issue" :rows="12" />
    <template v-else-if="issue">
      <header class="head">
        <div class="line">
          <TypeIcon :type="issue.issue_type" />
          <span class="key">{{ issue.key }}</span>
          <span class="type">{{ issue.issue_type }}</span>
          <span class="spacer" />
          <VButton :label="copied ? t('jira.issue.copied') : t('jira.issue.copyKey')" size="small" severity="secondary" text @click="copyKey" />
          <VButton :label="t('common.openInBrowser')" size="small" severity="secondary" @click="openUrl(issue.url)" />
        </div>
        <h1 class="title">{{ issue.summary }}</h1>
        <dl class="fields">
          <dt>{{ t('jira.issue.status') }}</dt>
          <dd><StatusChip :status="issue.status" :category="issue.status_category" /></dd>
          <dt>{{ t('jira.issue.priority') }}</dt>
          <dd><PriorityMark :priority="issue.priority" /></dd>
          <dt>{{ t('jira.issue.assignee') }}</dt>
          <dd>
            <span v-if="issue.assignee" class="person"><PersonAvatar :connector="issue.connector" :person="issue.assignee" small />{{ issue.assignee.name }}</span>
            <span v-else class="muted">{{ t('jira.issue.unassigned') }}</span>
          </dd>
          <dt>{{ t('jira.issue.reporter') }}</dt>
          <dd>
            <span v-if="issue.reporter" class="person"><PersonAvatar :connector="issue.connector" :person="issue.reporter" small />{{ issue.reporter.name }}</span>
            <span v-else class="muted">-</span>
          </dd>
          <dt>{{ t('jira.issue.updated') }}</dt>
          <dd :title="formatDate(issue.updated)">{{ formatRelative(issue.updated) }}</dd>
          <dt>{{ t('jira.issue.versions') }}</dt>
          <dd>
            <span v-for="version in issue.fix_versions" :key="version" class="tag">{{ version }}</span>
            <span v-if="!issue.fix_versions.length" class="muted">-</span>
          </dd>
          <dt>{{ t('jira.issue.components') }}</dt>
          <dd>
            <span v-for="component in issue.components" :key="component" class="tag">{{ component }}</span>
            <span v-if="!issue.components.length" class="muted">-</span>
          </dd>
          <dt>{{ t('jira.issue.labels') }}</dt>
          <dd>
            <span v-for="label in issue.labels" :key="label" class="tag">{{ label }}</span>
            <span v-if="!issue.labels.length" class="muted">-</span>
          </dd>
        </dl>
      </header>

      <section class="section">
        <h2>{{ t('jira.issue.description') }}</h2>
        <RichContent v-if="issue.description_html" :html="issue.description_html" :connector="issue.connector" />
        <p v-else class="muted">{{ t('jira.issue.noDescription') }}</p>
      </section>

      <section v-if="issue.subtasks.length" class="section">
        <h2>{{ t('jira.issue.subtasks') }} <span class="count">{{ issue.subtasks.length }}</span></h2>
        <button v-for="sub in issue.subtasks" :key="sub.key" class="row" @click="openKey(sub.key)">
          <TypeIcon :type="sub.issue_type" />
          <span class="row-key">{{ sub.key }}</span>
          <span class="row-text">{{ sub.summary }}</span>
          <StatusChip :status="sub.status" :category="sub.status_category" />
        </button>
      </section>

      <section v-if="issue.links.length" class="section">
        <h2>{{ t('jira.issue.links') }} <span class="count">{{ issue.links.length }}</span></h2>
        <button v-for="link in issue.links" :key="`${link.relation}-${link.key}`" class="row" @click="openKey(link.key)">
          <span class="relation">{{ link.relation }}</span>
          <span class="row-key">{{ link.key }}</span>
          <span class="row-text">{{ link.summary }}</span>
          <StatusChip :status="link.status" />
        </button>
      </section>

      <section class="section">
        <h2>{{ t('jira.issue.relatedMr') }}</h2>
        <slot name="related">
          <p class="muted">{{ t('jira.issue.noRelatedMr') }}</p>
        </slot>
      </section>

      <section class="section">
        <h2>{{ t('jira.issue.comments') }} <span class="count">{{ issue.comments.length }}</span></h2>
        <p v-if="!issue.comments.length" class="muted">{{ t('jira.issue.noComments') }}</p>
        <article v-for="comment in issue.comments" :key="comment.id" class="comment">
          <PersonAvatar :connector="issue.connector" :person="comment.author" />
          <div class="comment-main">
            <div class="comment-head">
              <strong>{{ comment.author.name }}</strong>
              <span class="muted" :title="formatDate(comment.created)">{{ formatRelative(comment.created) }}</span>
            </div>
            <RichContent :html="comment.body_html" :connector="issue.connector" />
          </div>
        </article>
      </section>
    </template>
  </div>
</template>

<style scoped>
.issue {
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: auto;
  background: var(--vr-color-surface);
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
}

.head,
.section {
  padding: var(--vr-space-4) var(--vr-space-6);
  border-bottom: 1px solid var(--vr-color-border);
}

.line {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
}

.key {
  font-family: var(--vr-font-mono);
  font-weight: 600;
}

.type,
.muted,
.count {
  color: var(--vr-color-text-muted);
}

.spacer {
  flex: 1;
}

.title {
  margin: var(--vr-space-3) 0;
  font-size: var(--vr-font-size-xl);
  font-weight: 600;
  overflow-wrap: anywhere;
}

.fields {
  display: grid;
  grid-template-columns: max-content 1fr max-content 1fr;
  gap: var(--vr-space-2) var(--vr-space-4);
  margin: 0;
}

.fields dt {
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
  line-height: 24px;
}

.fields dd {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-1);
  min-width: 0;
  margin: 0;
  min-height: 24px;
}

.person {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-2);
}

.tag {
  padding: 1px var(--vr-space-2);
  border-radius: var(--vr-radius-sm);
  background: var(--vr-color-chip-bg);
  font-size: var(--vr-font-size-xs);
}

.section h2 {
  margin: 0 0 var(--vr-space-3);
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
}

.row {
  display: flex;
  align-items: center;
  gap: var(--vr-space-3);
  width: 100%;
  height: var(--vr-size-row);
  padding: 0 var(--vr-space-2);
  border: 0;
  border-radius: var(--vr-radius-sm);
  background: transparent;
  color: var(--vr-color-text);
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.row:hover {
  background: var(--vr-color-nav-hover);
}

.row-key {
  flex: none;
  font-family: var(--vr-font-mono);
  color: var(--vr-color-text-muted);
}

.row-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.relation {
  flex: none;
  min-width: 110px;
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
}

.comment {
  display: flex;
  gap: var(--vr-space-3);
  padding: var(--vr-space-3) 0;
}

.comment + .comment {
  border-top: 1px solid var(--vr-color-nav-hover);
}

.comment-main {
  flex: 1;
  min-width: 0;
}

.comment-head {
  display: flex;
  gap: var(--vr-space-2);
  margin-bottom: var(--vr-space-1);
  font-size: var(--vr-font-size-xs);
}
</style>

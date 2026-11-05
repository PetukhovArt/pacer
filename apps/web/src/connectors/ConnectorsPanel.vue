<script setup lang="ts">
import { computed, onMounted, reactive, ref, shallowRef, type Component } from 'vue';
import { BookOpen, ExternalLink, Github, Gitlab, Plug, Plus, RefreshCw, Ticket } from 'lucide-vue-next';
import { VButton, VInputText, VMessage, VSelect } from '@ui/kit';
import { t } from '../i18n';
import AiAccounts from './AiAccounts.vue';
import PersonAvatar from './PersonAvatar.vue';
import { useAiAccounts } from './ai';
import { describeError, openUrl, useConnectors, type ConnectorKind, type ConnectorStatus, type ConnectorView, type Detected } from './api';

const { connectors, detected, loading, checking, error, check, add, update, remove, refresh } = useConnectors();
const ai = useAiAccounts();
const checkingAll = computed(() => checking.value || ai.checking.value);
const checkAll = () => Promise.all([check(), ai.check()]);

const KINDS: { value: ConnectorKind; label: string; icon: Component; cli: boolean }[] = [
  { value: 'github', label: 'GitHub', icon: Github, cli: true },
  { value: 'gitlab', label: 'GitLab', icon: Gitlab, cli: true },
  { value: 'jira', label: 'Jira', icon: Ticket, cli: false },
  { value: 'confluence', label: 'Confluence', icon: BookOpen, cli: false },
];

const statusText = (status: ConnectorStatus) => t(`connectors.status.${status}`);

const kindMeta = (kind: ConnectorKind) => KINDS.find((k) => k.value === kind)!;
const hostOf = (url: string) => {
  try {
    return new URL(url).host;
  } catch {
    return url.replace(/^https?:\/\//, '').replace(/\/.*$/, '');
  }
};
const isCloud = (kind: ConnectorKind, url: string) => (kind === 'jira' || kind === 'confluence') && /(^|\.)atlassian\.net$/i.test(hostOf(url.trim()).split(':')[0] ?? '');

function tokenPage(kind: ConnectorKind, url: string): string {
  const base = url.trim().replace(/\/+$/, '');
  if (isCloud(kind, url)) return 'https://id.atlassian.com/manage-profile/security/api-tokens';
  if (kind === 'github') return `${base || 'https://github.com'}/settings/tokens`;
  if (kind === 'gitlab') return `${base}/-/user_settings/personal_access_tokens`;
  if (kind === 'jira') return `${base}/secure/ViewProfile.jspa?selectedTab=com.atlassian.pats.pats-plugin:jira-user-personal-access-tokens`;
  return `${base}/plugins/personalaccesstokens/usertokens.action`;
}

const adding = shallowRef(false);
const form = reactive({ kind: 'github' as ConnectorKind, url: 'https://github.com', auth: 'cli' as 'cli' | 'token', token: '', email: '', label: '' });
const formError = shallowRef<string | null>(null);
const formBusy = shallowRef(false);

const canCli = computed(() => kindMeta(form.kind).cli);
const formCloud = computed(() => isCloud(form.kind, form.url));
const formTokenPage = computed(() => (form.url.trim() ? tokenPage(form.kind, form.url) : null));
const formValid = computed(() => form.url.trim().length > 0 && (form.auth === 'cli' || form.token.trim().length > 0) && (!formCloud.value || form.email.trim().length > 0));

function pickKind(kind: ConnectorKind) {
  form.kind = kind;
  form.url = kind === 'github' ? 'https://github.com' : '';
  form.auth = kindMeta(kind).cli ? 'cli' : 'token';
  form.token = '';
  form.email = '';
  formError.value = null;
}

function openForm() {
  pickKind('github');
  form.label = '';
  adding.value = true;
}

async function submit() {
  if (!formValid.value || formBusy.value) return;
  formBusy.value = true;
  formError.value = null;
  try {
    const baseUrl = form.url.trim().replace(/\/+$/, '');
    await add({
      kind: form.kind,
      baseUrl,
      label: form.label.trim() || hostOf(baseUrl),
      auth: form.auth,
      token: form.auth === 'token' ? form.token.trim() : null,
      email: formCloud.value ? form.email.trim() : null,
    });
    adding.value = false;
  } catch (e) {
    formError.value = describeError(e);
  } finally {
    formBusy.value = false;
  }
}

const detectBusy = shallowRef<string | null>(null);
const detectError = shallowRef<string | null>(null);
const detectKey = (d: Detected) => `${d.kind}:${d.base_url}`;

async function connectDetected(d: Detected) {
  detectBusy.value = detectKey(d);
  detectError.value = null;
  try {
    await add({ kind: d.kind, baseUrl: d.base_url, label: hostOf(d.base_url), auth: 'cli', token: null, email: null });
  } catch (e) {
    detectError.value = `${hostOf(d.base_url)}: ${describeError(e)}`;
  } finally {
    detectBusy.value = null;
  }
}

const editingId = shallowRef<string | null>(null);
const editForm = ref({ label: '', token: '', email: '' });
const editError = shallowRef<string | null>(null);
const editBusy = shallowRef(false);

function startEdit(c: ConnectorView) {
  editingId.value = c.id;
  editForm.value.label = c.label;
  editForm.value.token = '';
  editForm.value.email = '';
  editError.value = null;
}

async function saveEdit(c: ConnectorView) {
  editBusy.value = true;
  editError.value = null;
  try {
    await update({
      id: c.id,
      label: editForm.value.label.trim() || c.label,
      token: editForm.value.token.trim() || null,
      email: isCloud(c.kind, c.base_url) && editForm.value.email.trim() ? editForm.value.email.trim() : null,
    });
    editingId.value = null;
  } catch (e) {
    editError.value = describeError(e);
  } finally {
    editBusy.value = false;
  }
}

const confirmingId = shallowRef<string | null>(null);
const removeBusy = shallowRef(false);
const removeError = shallowRef<string | null>(null);

function askRemove(c: ConnectorView) {
  confirmingId.value = c.id;
  removeError.value = null;
}

async function confirmRemove(c: ConnectorView) {
  removeBusy.value = true;
  removeError.value = null;
  try {
    await remove(c.id);
    confirmingId.value = null;
  } catch (e) {
    removeError.value = describeError(e);
  } finally {
    removeBusy.value = false;
  }
}

onMounted(() => void refresh());
</script>

<template>
  <section class="connectors" data-testid="connectors-panel">
    <header class="head">
      <h2>{{ t('connectors.panel.title') }}</h2>
      <div class="head-actions">
        <VButton size="small" severity="secondary" text :label="t('connectors.panel.checkAll')" :loading="checkingAll" data-testid="connectors-check" @click="checkAll">
          <template #icon><RefreshCw :size="14" /></template>
        </VButton>
        <VButton v-if="!adding" size="small" :label="t('connectors.panel.add')" data-testid="connectors-add-open" @click="openForm">
          <template #icon><Plus :size="14" /></template>
        </VButton>
      </div>
    </header>

    <VMessage v-if="error" severity="error" size="small" data-testid="connectors-error">{{ error }}</VMessage>

    <AiAccounts />

    <h3 class="group-title">{{ t('connectors.panel.gitGroup') }}</h3>

    <div v-if="detected.length" v-auto-animate class="block" data-testid="connectors-detected">
      <h3>{{ t('connectors.panel.detectedTitle') }}</h3>
      <div v-for="d in detected" :key="detectKey(d)" class="detected" :data-testid="`detected-${d.kind}`">
        <component :is="kindMeta(d.kind).icon" :size="16" class="kind-icon" />
        <span class="detected-main">
          <b>{{ kindMeta(d.kind).label }}</b>
          <span class="muted">{{ hostOf(d.base_url) }}</span>
          <span v-if="d.account" class="muted">· {{ d.account }}</span>
        </span>
        <VButton size="small" :label="t('connectors.panel.connect')" :loading="detectBusy === detectKey(d)" :disabled="detectBusy !== null" data-testid="detected-connect" @click="connectDetected(d)" />
      </div>
      <VMessage v-if="detectError" severity="error" size="small">{{ detectError }}</VMessage>
    </div>

    <form v-if="adding" class="block form" data-testid="connector-form" @submit.prevent="submit">
      <h3>{{ t('connectors.panel.newTitle') }}</h3>
      <div class="kinds" role="radiogroup" :aria-label="t('connectors.panel.typeLabel')">
        <button
          v-for="k in KINDS"
          :key="k.value"
          type="button"
          class="kind"
          role="radio"
          :aria-checked="form.kind === k.value"
          :data-active="form.kind === k.value"
          :data-testid="`kind-${k.value}`"
          @click="pickKind(k.value)"
        >
          <component :is="k.icon" :size="14" />{{ k.label }}
        </button>
      </div>

      <label class="field">
        <span>{{ t('connectors.panel.url') }}</span>
        <VInputText v-model="form.url" placeholder="https://host" autocomplete="off" data-testid="connector-url" />
      </label>

      <div v-if="canCli" class="field">
        <span>{{ t('connectors.panel.signIn') }}</span>
        <VSelect
          v-model="form.auth"
          :options="[
            { value: 'cli', label: form.kind === 'github' ? t('connectors.panel.signInGh') : t('connectors.panel.signInGlab') },
            { value: 'token', label: t('connectors.panel.token') },
          ]"
          option-label="label"
          option-value="value"
          data-testid="connector-auth"
        />
      </div>

      <label v-if="form.auth === 'token'" class="field">
        <span>{{ t('connectors.panel.token') }}</span>
        <VInputText v-model="form.token" type="password" autocomplete="new-password" :placeholder="t('connectors.panel.tokenPlaceholder')" data-testid="connector-token" />
        <a v-if="formTokenPage" class="link" :href="formTokenPage" data-testid="connector-token-help" @click.prevent="openUrl(formTokenPage)">
          {{ t('connectors.panel.howToGetToken') }} <ExternalLink :size="12" />
        </a>
      </label>

      <label v-if="formCloud" class="field">
        <span>{{ t('connectors.panel.atlassianEmail') }}</span>
        <VInputText v-model="form.email" type="email" autocomplete="off" placeholder="you@company.com" data-testid="connector-email" />
      </label>

      <label class="field">
        <span>{{ t('connectors.panel.name') }}</span>
        <VInputText v-model="form.label" :placeholder="hostOf(form.url) || t('connectors.panel.name')" autocomplete="off" data-testid="connector-label" />
      </label>

      <VMessage v-if="formError" severity="error" size="small" data-testid="connector-form-error">{{ formError }}</VMessage>

      <div class="form-actions">
        <VButton type="submit" size="small" :label="t('connectors.panel.connect')" :loading="formBusy" :disabled="!formValid" data-testid="connector-submit" />
        <VButton type="button" size="small" :label="t('common.cancel')" severity="secondary" text @click="adding = false" />
      </div>
    </form>

    <p v-if="!connectors.length && !loading && !adding" class="empty" data-testid="connectors-empty">
      <Plug :size="18" />{{ t('connectors.panel.empty') }}
    </p>

    <ul v-auto-animate class="cards">
      <li v-for="c in connectors" :key="c.id" class="card" :data-testid="`connector-${c.id}`" :data-status="c.status">
        <div class="card-row">
          <component :is="kindMeta(c.kind).icon" :size="20" class="kind-icon" />
          <div class="card-main">
            <div class="card-title">
              <b>{{ c.label }}</b>
              <span class="chip" :data-status="c.status" data-testid="connector-status">{{ statusText(c.status) }}</span>
            </div>
            <div class="muted host">{{ hostOf(c.base_url) }} · {{ c.auth === 'cli' ? t('connectors.panel.authCli') : t('connectors.panel.authToken') }}</div>
          </div>
          <div v-if="c.account" class="account">
            <PersonAvatar :person="c.account" small />
            <span>{{ c.account.name || c.account.username }}</span>
          </div>
        </div>

        <p v-if="c.error && c.status !== 'ok'" class="card-error" role="alert" data-testid="connector-error">{{ c.error }}</p>

        <form v-if="editingId === c.id" class="edit" @submit.prevent="saveEdit(c)">
          <label class="field">
            <span>{{ t('connectors.panel.name') }}</span>
            <VInputText v-model="editForm.label" autocomplete="off" />
          </label>
          <label v-if="c.auth === 'token'" class="field">
            <span>{{ t('connectors.panel.newToken') }}</span>
            <VInputText v-model="editForm.token" type="password" autocomplete="new-password" :placeholder="t('connectors.panel.keepEmpty')" />
            <a class="link" :href="tokenPage(c.kind, c.base_url)" @click.prevent="openUrl(tokenPage(c.kind, c.base_url))">
              {{ t('connectors.panel.howToGetToken') }} <ExternalLink :size="12" />
            </a>
          </label>
          <label v-if="isCloud(c.kind, c.base_url)" class="field">
            <span>{{ t('connectors.panel.atlassianEmail') }}</span>
            <VInputText v-model="editForm.email" type="email" autocomplete="off" :placeholder="t('connectors.panel.keepEmpty')" />
          </label>
          <VMessage v-if="editError" severity="error" size="small">{{ editError }}</VMessage>
          <div class="form-actions">
            <VButton type="submit" size="small" :label="t('common.save')" :loading="editBusy" />
            <VButton type="button" size="small" :label="t('common.cancel')" severity="secondary" text @click="editingId = null" />
          </div>
        </form>

        <div v-else-if="confirmingId === c.id" class="confirm" role="group" :aria-label="t('connectors.panel.disconnectConfirmLabel')" data-testid="connector-remove-confirm">
          <span>{{ t('connectors.panel.disconnectConfirm', { name: c.label }) }}</span>
          <VButton size="small" severity="danger" :label="t('connectors.panel.disconnect')" :loading="removeBusy" data-testid="connector-remove-yes" @click="confirmRemove(c)" />
          <VButton size="small" severity="secondary" text :label="t('common.cancel')" @click="confirmingId = null" />
          <span v-if="removeError" class="card-error">{{ removeError }}</span>
        </div>

        <div v-else class="card-actions">
          <VButton size="small" severity="secondary" text :label="t('connectors.panel.check')" :disabled="checking" data-testid="connector-check" @click="check" />
          <VButton size="small" severity="secondary" text :label="t('connectors.panel.edit')" data-testid="connector-edit" @click="startEdit(c)" />
          <VButton size="small" severity="danger" text :label="t('connectors.panel.disconnect')" data-testid="connector-remove" @click="askRemove(c)" />
        </div>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.connectors {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
  width: 100%;
  height: 100%;
  padding: var(--vr-space-4);
  overflow: auto;
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--vr-space-2);
}

h2 {
  margin: 0;
  font-size: var(--vr-font-size-lg);
}

h3 {
  margin: 0 0 var(--vr-space-2);
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-muted);
}

.group-title {
  margin: var(--vr-space-2) 0 0;
}

.head-actions {
  display: flex;
  gap: var(--vr-space-1);
}

.block,
.card {
  padding: var(--vr-space-3);
  background: var(--vr-color-surface-muted);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
}

.detected {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-height: var(--vr-size-row);
}

.detected-main {
  display: flex;
  flex: 1;
  gap: var(--vr-space-1);
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
}

.kind-icon {
  flex: none;
  color: var(--vr-color-text-muted);
}

.muted {
  color: var(--vr-color-text-muted);
}

.kinds {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-1);
  margin-bottom: var(--vr-space-2);
}

.kind {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  padding: var(--vr-space-1) var(--vr-space-2);
  color: var(--vr-color-text-muted);
  font: inherit;
  background: none;
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-sm);
  cursor: pointer;
}

.kind[data-active='true'] {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-active);
}

.field {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
  margin-bottom: var(--vr-space-2);
}

.field > span:first-child {
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
}

.link {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  align-self: flex-start;
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-xs);
  text-decoration: underline;
  cursor: pointer;
}

.form-actions,
.card-actions,
.confirm {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-1);
}

.empty {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  margin: 0;
  color: var(--vr-color-text-muted);
}

.cards {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.card-row {
  display: flex;
  align-items: center;
  gap: var(--vr-space-3);
}

.card-main {
  flex: 1;
  min-width: 0;
}

.card-title {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
}

.host {
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.account {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
}

.chip {
  padding: 1px var(--vr-space-2);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-2xs);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.chip[data-status='ok'] {
  color: var(--vr-color-success);
}

.chip[data-status='error'] {
  color: var(--vr-color-danger);
}

.chip[data-status='unreachable'] {
  color: var(--vr-color-conflict);
}

.card-error {
  margin: var(--vr-space-2) 0 0;
  color: var(--vr-color-danger);
  font-size: var(--vr-font-size-xs);
  word-break: break-word;
}

.card-actions,
.edit,
.confirm {
  margin-top: var(--vr-space-2);
}
</style>

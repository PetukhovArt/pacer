<script setup lang="ts">
import { computed, onMounted, shallowRef, type Component } from 'vue';
import { Bot, Brain, Cpu, ExternalLink, Gem, KeyRound, Sparkles, Waypoints, Zap } from 'lucide-vue-next';
import { VButton, VInputText, VMessage } from '@ui/kit';
import { useI18n } from '../i18n';
import { describeError, openUrl } from './api';
import { useAiAccounts, type AiKind, type AiStatus, type AiView } from './ai';

const { t, formatDate } = useI18n();
const { accounts, pending, problems, refresh, recheck, login, logout, cancel, saveKey, removeKey } = useAiAccounts();

const ICONS: Record<AiKind, Component> = {
  claude: Sparkles,
  codex: Bot,
  gemini: Gem,
  anthropic_api: Brain,
  openai_api: Cpu,
  openrouter: Waypoints,
  grok: Zap,
};

const KEY_PAGES: Partial<Record<AiKind, string>> = {
  anthropic_api: 'https://console.anthropic.com/settings/keys',
  openai_api: 'https://platform.openai.com/api-keys',
  openrouter: 'https://openrouter.ai/keys',
  grok: 'https://console.x.ai',
};

const BINARIES: Partial<Record<AiKind, string>> = { claude: 'claude', codex: 'codex', gemini: 'gemini' };

function statusText(a: AiView): string {
  const key = a.method === 'key';
  const texts: Record<AiStatus, string> = {
    signed_in: key ? t('connectors.ai.status.keyAccepted') : t('connectors.ai.status.signedIn'),
    signed_out: key ? t('connectors.ai.status.noKey') : t('connectors.ai.status.signedOut'),
    not_installed: t('connectors.ai.status.notInstalled'),
    rejected: t('connectors.ai.status.rejected'),
    unreachable: t('connectors.ai.status.unreachable'),
    error: t('connectors.ai.status.error'),
    unchecked: t('connectors.ai.status.unchecked'),
  };
  return texts[a.status];
}

function chipState(status: AiStatus): string {
  if (status === 'signed_in') return 'ok';
  if (status === 'rejected' || status === 'error') return 'error';
  if (status === 'unreachable') return 'unreachable';
  return 'idle';
}

function subtitle(a: AiView): string {
  if (a.method === 'key') return t('connectors.ai.methodKey');
  return a.version ? `CLI · ${a.version}` : 'CLI';
}

function identity(a: AiView): string {
  return [a.account, a.plan].filter(Boolean).join(' · ');
}

function expiry(a: AiView): string | null {
  if (a.status !== 'signed_in' || !a.expires_at) return null;
  const date = new Date(a.expires_at);
  if (Number.isNaN(date.getTime())) return null;
  return formatDate(date, { day: '2-digit', month: '2-digit', hour: '2-digit', minute: '2-digit' });
}

const hasKey = (a: AiView) => a.method === 'key' && a.status !== 'signed_out';
const canLogin = (a: AiView) => a.method === 'cli' && a.status !== 'not_installed' && a.status !== 'signed_in';
const canLogout = (a: AiView) => a.method === 'cli' && a.status === 'signed_in';
const gemini = (a: AiView) => a.kind === 'gemini';

const confirmingLogout = shallowRef<AiKind | null>(null);
const confirmingRemove = shallowRef<AiKind | null>(null);
const keyKind = shallowRef<AiKind | null>(null);
const keyValue = shallowRef('');
const keyBusy = shallowRef(false);
const keyError = shallowRef<string | null>(null);
const keyPage = computed(() => (keyKind.value ? (KEY_PAGES[keyKind.value] ?? null) : null));

function openKeyForm(a: AiView) {
  keyKind.value = a.kind;
  keyValue.value = '';
  keyError.value = null;
  confirmingRemove.value = null;
}

async function submitKey() {
  const kind = keyKind.value;
  const key = keyValue.value.trim();
  if (!kind || !key || keyBusy.value) return;
  keyBusy.value = true;
  keyError.value = null;
  try {
    await saveKey(kind, key);
    keyKind.value = null;
    keyValue.value = '';
  } catch (e) {
    keyError.value = describeError(e);
  } finally {
    keyBusy.value = false;
  }
}

async function confirmLogout(a: AiView) {
  confirmingLogout.value = null;
  await logout(a.kind);
}

async function confirmRemove(a: AiView) {
  confirmingRemove.value = null;
  await removeKey(a.kind);
}

onMounted(() => void refresh());
</script>

<template>
  <section class="group" data-testid="ai-accounts">
    <h3>{{ t('connectors.ai.title') }}</h3>
    <ul v-auto-animate class="cards">
      <li v-for="a in accounts" :key="a.kind" class="card" :data-testid="`ai-${a.kind}`" :data-status="a.status">
        <div class="card-row">
          <component :is="ICONS[a.kind]" :size="20" class="kind-icon" />
          <div class="card-main">
            <div class="card-title">
              <b>{{ a.label }}</b>
              <span class="chip" :data-state="chipState(a.status)" data-testid="ai-status">{{ statusText(a) }}</span>
            </div>
            <div class="muted host">
              {{ subtitle(a) }}<template v-if="expiry(a)"> · {{ t('connectors.ai.expiresUntil', { date: expiry(a) ?? '' }) }}</template>
            </div>
          </div>
          <div v-if="identity(a)" class="account" data-testid="ai-account">{{ identity(a) }}</div>
        </div>

        <p v-if="a.detail && a.status !== 'signed_in'" class="card-error" role="alert" data-testid="ai-detail">{{ a.detail }}</p>
        <p v-if="problems[a.kind]" class="card-error" role="alert" data-testid="ai-problem">{{ problems[a.kind] }}</p>
        <p v-if="a.status === 'not_installed'" class="hint">{{ t('connectors.ai.installBefore') }} <code>{{ BINARIES[a.kind] }}</code>{{ t('connectors.ai.installAfter') }}</p>

        <div v-if="pending[a.kind]" class="confirm" role="status" data-testid="ai-waiting">
          <span>{{ pending[a.kind] === 'login' ? t('connectors.ai.waitingLogin') : t('connectors.ai.waitingLogout') }}</span>
          <VButton size="small" severity="secondary" text :label="t('connectors.ai.stopWaiting')" data-testid="ai-cancel" @click="cancel(a.kind)" />
        </div>

        <form v-else-if="keyKind === a.kind" class="edit" @submit.prevent="submitKey">
          <label class="field">
            <span>{{ t('connectors.ai.apiKey') }}</span>
            <VInputText v-model="keyValue" type="password" autocomplete="new-password" :placeholder="t('connectors.ai.keyPlaceholder')" data-testid="ai-key-input" />
            <a v-if="keyPage" class="link" :href="keyPage" @click.prevent="openUrl(keyPage)"> {{ t('connectors.ai.whereToGetKey') }} <ExternalLink :size="12" /> </a>
          </label>
          <VMessage v-if="keyError" severity="error" size="small" data-testid="ai-key-error">{{ keyError }}</VMessage>
          <div class="form-actions">
            <VButton type="submit" size="small" :label="t('connectors.ai.checkAndSave')" :loading="keyBusy" :disabled="!keyValue.trim()" data-testid="ai-key-save" />
            <VButton type="button" size="small" :label="t('common.cancel')" severity="secondary" text @click="keyKind = null" />
          </div>
        </form>

        <div v-else-if="confirmingLogout === a.kind" class="confirm" role="group" :aria-label="t('connectors.ai.logoutConfirmLabel')" data-testid="ai-logout-confirm">
          <span v-if="gemini(a)">{{ t('connectors.ai.logoutGemini', { name: a.label }) }}</span>
          <span v-else>{{ t('connectors.ai.logoutCli', { name: a.label, command: a.command?.replace(/ login$/, ' logout') ?? '' }) }}</span>
          <VButton size="small" severity="danger" :label="t('connectors.ai.signOut')" data-testid="ai-logout-yes" @click="confirmLogout(a)" />
          <VButton size="small" severity="secondary" text :label="t('common.cancel')" @click="confirmingLogout = null" />
        </div>

        <div v-else-if="confirmingRemove === a.kind" class="confirm" role="group" :aria-label="t('connectors.ai.removeKeyConfirmLabel')" data-testid="ai-remove-confirm">
          <span>{{ t('connectors.ai.removeKeyConfirm', { name: a.label }) }}</span>
          <VButton size="small" severity="danger" :label="t('common.delete')" data-testid="ai-remove-yes" @click="confirmRemove(a)" />
          <VButton size="small" severity="secondary" text :label="t('common.cancel')" @click="confirmingRemove = null" />
        </div>

        <div v-else class="card-actions">
          <VButton v-if="canLogin(a)" size="small" :label="t('connectors.ai.signIn')" data-testid="ai-login" @click="login(a.kind)" />
          <VButton v-if="canLogout(a)" size="small" severity="secondary" :label="t('connectors.ai.signOut')" data-testid="ai-logout" @click="confirmingLogout = a.kind" />
          <VButton v-if="a.method === 'key' && !hasKey(a)" size="small" :label="t('connectors.ai.addKey')" data-testid="ai-key-open" @click="openKeyForm(a)">
            <template #icon><KeyRound :size="14" /></template>
          </VButton>
          <VButton v-if="hasKey(a)" size="small" severity="secondary" text :label="t('connectors.ai.replaceKey')" data-testid="ai-key-replace" @click="openKeyForm(a)" />
          <VButton v-if="hasKey(a)" size="small" severity="danger" text :label="t('connectors.ai.removeKey')" data-testid="ai-key-remove" @click="confirmingRemove = a.kind" />
          <VButton size="small" severity="secondary" text :label="t('connectors.ai.check')" data-testid="ai-check" @click="recheck(a.kind)" />
        </div>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.group {
  display: flex;
  flex-direction: column;
}

h3 {
  margin: 0 0 var(--vr-space-2);
  font-size: var(--vr-font-size-sm);
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

.card {
  padding: var(--vr-space-3);
  background: var(--vr-color-surface-muted);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
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

.kind-icon {
  flex: none;
  color: var(--vr-color-text-muted);
}

.muted {
  color: var(--vr-color-text-muted);
}

.host {
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.account {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.chip {
  padding: 1px var(--vr-space-2);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-2xs);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.chip[data-state='ok'] {
  color: var(--vr-color-success);
}

.chip[data-state='error'] {
  color: var(--vr-color-danger);
}

.chip[data-state='unreachable'] {
  color: var(--vr-color-conflict);
}

.card-error {
  margin: var(--vr-space-2) 0 0;
  color: var(--vr-color-danger);
  font-size: var(--vr-font-size-xs);
  word-break: break-word;
}

.hint {
  margin: var(--vr-space-2) 0 0;
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
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

.card-actions,
.edit,
.confirm {
  margin-top: var(--vr-space-2);
}
</style>

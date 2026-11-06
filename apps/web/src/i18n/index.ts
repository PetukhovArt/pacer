import { computed, watch } from 'vue';
import { useSettings } from '../settings/store';
import { en } from './en';
import { ru } from './ru';
import type { KeyPaths, Params, PluralEn, PluralKeyPaths, PluralRu } from './types';

export type Locale = 'en' | 'ru';
export type MessageKey = KeyPaths<typeof en>;
type PluralKey = PluralKeyPaths<typeof en>;
type Entry = string | PluralEn | PluralRu;

const CATALOGS: Record<Locale, unknown> = { en, ru };
const INTL_TAG: Record<Locale, string> = { en: 'en-US', ru: 'ru-RU' };

const settings = useSettings();

const locale = computed<Locale>(() => ((settings.values.value as Record<string, unknown>)['ui.language'] === 'ru' ? 'ru' : 'en'));

function applyLang(value: Locale): void {
  if (typeof document !== 'undefined') document.documentElement.lang = value;
}

applyLang(locale.value);
watch(locale, applyLang);

function lookup(catalog: unknown, key: string): Entry | undefined {
  let node: unknown = catalog;
  for (const part of key.split('.')) {
    if (typeof node !== 'object' || node === null) return undefined;
    node = (node as Record<string, unknown>)[part];
  }
  return node as Entry | undefined;
}

function interpolate(template: string, params: Params | undefined): string {
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (match, name: string) => (name in params ? String(params[name]) : match));
}

export function t(key: Exclude<MessageKey, PluralKey>, params?: Params): string;
export function t(key: PluralKey, params: Params & { count: number }): string;
export function t(key: MessageKey, params?: Params): string {
  const active = locale.value;
  const entry = lookup(CATALOGS[active], key) ?? lookup(CATALOGS.en, key);
  if (entry === undefined) return key;
  if (typeof entry === 'string') return interpolate(entry, params);
  const forms = entry as Record<string, string>;
  const category = new Intl.PluralRules(INTL_TAG[active]).select(Number(params?.count ?? 0));
  return interpolate(forms[category] ?? forms.other ?? key, params);
}

const RELATIVE_UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ['year', 31536000],
  ['month', 2592000],
  ['week', 604800],
  ['day', 86400],
  ['hour', 3600],
  ['minute', 60],
];

function toTime(value: Date | string | number): number {
  return value instanceof Date ? value.getTime() : typeof value === 'number' ? value : Date.parse(value);
}

function setLocale(next: Locale): void {
  settings.set('ui.language', next);
}

function formatRelative(value: Date | string | number): string {
  const time = toTime(value);
  if (Number.isNaN(time)) return '';
  const seconds = Math.round((time - Date.now()) / 1000);
  const abs = Math.abs(seconds);
  if (abs < 45) return t('common.time.justNow');
  const format = new Intl.RelativeTimeFormat(INTL_TAG[locale.value], { numeric: 'auto' });
  for (const [unit, size] of RELATIVE_UNITS) {
    if (abs >= size) return format.format(Math.round(seconds / size), unit);
  }
  return format.format(Math.round(seconds / 60), 'minute');
}

function formatDate(value: Date | string | number, options: Intl.DateTimeFormatOptions = { dateStyle: 'medium', timeStyle: 'short' }): string {
  const time = toTime(value);
  if (Number.isNaN(time)) return typeof value === 'string' ? value : '';
  return new Intl.DateTimeFormat(INTL_TAG[locale.value], options).format(time);
}

function formatNumber(value: number, options?: Intl.NumberFormatOptions): string {
  return new Intl.NumberFormat(INTL_TAG[locale.value], options).format(value);
}

function formatBytes(bytes: number): string {
  const megabytes = bytes / (1024 * 1024);
  const gigabyte = megabytes >= 1024;
  return new Intl.NumberFormat(INTL_TAG[locale.value], {
    style: 'unit',
    unit: gigabyte ? 'gigabyte' : 'megabyte',
    unitDisplay: 'short',
    maximumFractionDigits: gigabyte ? 1 : 0,
  }).format(gigabyte ? megabytes / 1024 : megabytes);
}

const api = { t, locale, setLocale, formatRelative, formatDate, formatNumber, formatBytes };

export function useI18n(): typeof api {
  return api;
}

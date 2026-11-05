import autoAnimate, { type AnimationController, type AutoAnimateOptions } from '@formkit/auto-animate';
import type { App, Directive } from 'vue';

const REDUCE_MOTION_CLASS = 'vr-reduce-motion';

type AnimateValue = Partial<AutoAnimateOptions> | boolean | undefined;

const controllers = new Map<HTMLElement, AnimationController | null>();
const options = new Map<HTMLElement, Partial<AutoAnimateOptions>>();
const reduced = typeof matchMedia === 'function' ? matchMedia('(prefers-reduced-motion: reduce)') : null;
let forcedOff = false;

export function isMotionEnabled(): boolean {
  return !forcedOff && !reduced?.matches;
}

const toMs = (value: string, fallback: number): number => {
  const number = parseFloat(value);
  if (Number.isNaN(number)) return fallback;
  return value.endsWith('ms') ? number : number * 1000;
};

const token = (name: string): string => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

const start = (el: HTMLElement): void => {
  const easing = token('--vr-motion-ease-standard') || 'ease-out';
  controllers.set(el, autoAnimate(el, { duration: toMs(token('--vr-motion-base'), 160), easing, ...options.get(el) }));
};

const sync = (): void => {
  const on = isMotionEnabled();
  for (const [el, controller] of controllers) {
    if (!on) controller?.disable();
    else if (controller) controller.enable();
    else start(el);
  }
};

export function setMotionEnabled(on: boolean): void {
  forcedOff = !on;
  document.documentElement.classList.toggle(REDUCE_MOTION_CLASS, forcedOff);
  sync();
}

const autoAnimateDirective: Directive<HTMLElement, AnimateValue> = {
  mounted(el, binding) {
    if (binding.value === false) return;
    if (typeof binding.value === 'object') options.set(el, binding.value);
    controllers.set(el, null);
    if (isMotionEnabled()) start(el);
  },
  unmounted(el) {
    controllers.get(el)?.disable();
    controllers.delete(el);
    options.delete(el);
  },
};

declare module 'vue' {
  interface GlobalDirectives {
    vAutoAnimate: typeof autoAnimateDirective;
  }
}

export function installMotion(app: App): void {
  app.directive('auto-animate', autoAnimateDirective);
  reduced?.addEventListener('change', sync);
}

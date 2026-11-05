<script setup lang="ts">
import '@xterm/xterm/css/xterm.css';
import { onBeforeUnmount, onMounted, useTemplateRef, watch } from 'vue';
import { Terminal, type ITheme } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { Unicode11Addon } from '@xterm/addon-unicode11';
import { WebglAddon } from '@xterm/addon-webgl';
import { attachSession, detachSession, resizeSession, sendInput, sessionId, type SessionEvent, type SessionRef } from './pacer';
import { t } from '../i18n';
import { applyCtrl } from '../mobile/keys';
import { useSettings } from '../settings/store';
import { zoomTerminal } from './terminalFont';

const props = defineProps<{ session: SessionRef }>();
const host = useTemplateRef<HTMLDivElement>('host');
const { values } = useSettings();

const ESC = '\x1b';
const SHIFT_ENTER = `${ESC}\r`;
const CTRL_ENTER = `${ESC}[13;5u`;
const REPORT_BODY = /^(?:\??\d+;\d+R|[?>][\d;]*c|\??[\d;]*n)$/;
const RESIZE_DEBOUNCE_MS = 80;

const CONPTY_ANSWERS_QUERIES = navigator.userAgent.includes('Windows');

const isTerminalReport = (data: string) => CONPTY_ANSWERS_QUERIES && data.startsWith(`${ESC}[`) && REPORT_BODY.test(data.slice(2));

function enterSequence(event: KeyboardEvent): string | null {
  if (event.key !== 'Enter' || event.altKey || event.metaKey || event.shiftKey === event.ctrlKey) return null;
  return event.shiftKey ? SHIFT_ENTER : CTRL_ENTER;
}

function decode(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

function themeFromTokens(): ITheme {
  const style = getComputedStyle(document.documentElement);
  const token = (name: string) => style.getPropertyValue(`--vr-color-${name}`).trim();
  return { background: token('surface'), foreground: token('text'), cursor: token('text'), cursorAccent: token('surface'), selectionBackground: token('nav-active') };
}

let teardown = () => {};

onMounted(() => {
  const element = host.value;
  if (!element) return;
  const session = props.session;
  const term = new Terminal({
    allowProposedApi: true,
    fontFamily: values.value['terminal.font_family'],
    fontSize: values.value['terminal.font_size'],
    lineHeight: values.value['terminal.line_height'],
    cursorBlink: values.value['terminal.cursor_blink'],
    scrollback: values.value['terminal.scrollback'],
    theme: themeFromTokens(),
  });
  const fit = new FitAddon();
  term.loadAddon(fit);
  term.loadAddon(new Unicode11Addon());
  term.unicode.activeVersion = '11';
  term.open(element);
  const webgl = new WebglAddon();
  webgl.onContextLoss(() => webgl.dispose());
  try {
    term.loadAddon(webgl);
  } catch {
    webgl.dispose();
  }

  let disposed = false;
  let fromKeyboard = false;
  const input = (data: string) => void sendInput(session, applyCtrl(data));

  term.attachCustomKeyEventHandler((event) => {
    fromKeyboard = true;
    queueMicrotask(() => {
      fromKeyboard = false;
    });
    const sequence = enterSequence(event);
    if (sequence) {
      if (event.type === 'keydown') {
        event.preventDefault();
        input(sequence);
      }
      return false;
    }
    if (event.type !== 'keydown' || !event.ctrlKey || event.shiftKey || event.altKey || event.metaKey) return true;
    if (event.code === 'KeyV') return false;
    if (event.code === 'KeyC') return !term.hasSelection();
    return true;
  });

  term.onData((data) => {
    if (!fromKeyboard && isTerminalReport(data)) return;
    input(data);
  });
  term.onResize(({ cols, rows }) => void resizeSession(session, cols, rows));

  const refit = () => {
    if (element.clientWidth && element.clientHeight) fit.fit();
  };
  let timer: ReturnType<typeof setTimeout> | undefined;
  const resizes = new ResizeObserver(() => {
    clearTimeout(timer);
    timer = setTimeout(refit, RESIZE_DEBOUNCE_MS);
  });
  resizes.observe(element);
  const themes = new MutationObserver(() => {
    term.options.theme = themeFromTokens();
  });
  themes.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] });

  const stopLayout = watch(
    () => [values.value['terminal.font_family'], values.value['terminal.font_size'], values.value['terminal.line_height']] as const,
    ([fontFamily, fontSize, lineHeight]) => {
      term.options.fontFamily = fontFamily;
      term.options.fontSize = fontSize;
      term.options.lineHeight = lineHeight;
      refit();
    },
  );
  const stopBehaviour = watch(
    () => [values.value['terminal.cursor_blink'], values.value['terminal.scrollback']] as const,
    ([cursorBlink, scrollback]) => {
      term.options.cursorBlink = cursorBlink;
      term.options.scrollback = scrollback;
    },
  );
  const onWheel = (event: WheelEvent) => {
    if (!event.ctrlKey || event.deltaY === 0) return;
    event.preventDefault();
    event.stopPropagation();
    zoomTerminal(event.deltaY < 0 ? 1 : -1);
  };
  element.addEventListener('wheel', onWheel, { passive: false, capture: true });

  refit();
  const onEvent = (event: SessionEvent) => {
    if (disposed) return;
    switch (event.type) {
      case 'reset':
        term.reset();
        term.write(decode(event.data));
        return;
      case 'output':
        term.write(decode(event.data));
        return;
      case 'exited':
        term.write(`\r\n${ESC}[2m[${event.exit_code === null ? t('agents.session.exited') : t('agents.session.exitedWithCode', { code: event.exit_code })}]${ESC}[0m\r\n`);
        return;
      default: {
        const unknown: never = event;
        return unknown;
      }
    }
  };
  const attached = attachSession(session, term.cols, term.rows, onEvent);
  term.focus();

  teardown = () => {
    disposed = true;
    clearTimeout(timer);
    resizes.disconnect();
    themes.disconnect();
    stopLayout();
    stopBehaviour();
    element.removeEventListener('wheel', onWheel, { capture: true });
    void attached.then(() => detachSession(session));
    term.dispose();
  };
});

onBeforeUnmount(() => teardown());
</script>

<template>
  <div class="frame">
    <div ref="host" class="terminal" data-testid="session-terminal" :data-session-id="sessionId(session)" />
  </div>
</template>

<style scoped>
.frame {
  box-sizing: border-box;
  width: 100%;
  height: 100%;
  padding: var(--vr-space-1) 0 0 var(--vr-space-2);
  background: var(--vr-color-surface);
}

.terminal {
  width: 100%;
  height: 100%;
  overflow: hidden;
}
</style>

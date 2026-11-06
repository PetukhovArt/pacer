import type { Translation } from '../types';
import type { content as en } from '../en/content';

export const content: Translation<typeof en> = {
  copy: 'Копировать',
  copyCode: 'Копировать код',
  sectionLink: 'Ссылка на раздел',
  alert: {
    note: 'Заметка',
    tip: 'Совет',
    important: 'Важно',
    warning: 'Предупреждение',
    caution: 'Осторожно',
  },
  mermaid: {
    copied: 'Скопировано',
    copyFailed: 'Не удалось',
    copySvg: 'Копировать SVG',
    unknownError: 'неизвестная ошибка',
    zoomOut: 'Уменьшить',
    zoomIn: 'Увеличить',
    fit: 'Вписать в ширину',
    hint: 'Колесо — прокрутка, Ctrl + колесо — масштаб, перетаскивание — сдвиг',
    fullscreen: 'На весь экран',
    close: 'Закрыть (Esc)',
    source: 'Исходник',
    failed: 'Диаграмма не отрисована',
    loading: 'Рисуем диаграмму…',
  },
};

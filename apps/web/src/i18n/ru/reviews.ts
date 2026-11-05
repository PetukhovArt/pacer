import type { Translation } from '../types';
import type { reviews as en } from '../en/reviews';

export const reviews: Translation<typeof en> = {
  title: 'Ревью',
  empty: 'Подключите GitHub или GitLab, чтобы видеть здесь MR и PR',
  placeholder: 'Выберите MR/PR слева',
  connectorPage: {
    connection: 'Подключение',
    connect: 'Подключить',
    listWidth: 'Ширина списка',
  },
};

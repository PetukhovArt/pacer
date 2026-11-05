import type { Translation } from '../types';
import type { reviews as en } from '../en/reviews';

export const reviews: Translation<typeof en> = {
  connectorPage: {
    connection: 'Подключение',
    connect: 'Подключить',
    listWidth: 'Ширина списка',
  },
};

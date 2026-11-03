import primitive from '../tokens/primitive.json' with { type: 'json' };

const ink = primitive.palette.ink;

export const inkScale = Object.fromEntries(Object.entries(ink).map(([step, token]) => [step, token.$value])) as Record<keyof typeof ink, string>;

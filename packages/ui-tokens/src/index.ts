import primitive from '../tokens/primitive.json' with { type: 'json' };

type Scale = Record<'50' | '100' | '200' | '300' | '400' | '500' | '600' | '700' | '800' | '900' | '950', { $value: string }>;

function values(scale: Scale) {
  return Object.fromEntries(Object.entries(scale).map(([step, token]) => [step, token.$value]));
}

export const accentScale = values(primitive.palette.indigo);
export const surfaceScale = { 0: primitive.palette.slate['0'].$value, ...values(primitive.palette.slate) };

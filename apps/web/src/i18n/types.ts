export type PluralEn = { readonly one: string; readonly other: string };
export type PluralRu = { readonly one: string; readonly few: string; readonly many: string; readonly other: string };

export type Translation<T> = {
  [K in keyof T]: T[K] extends string ? string : T[K] extends PluralEn ? PluralRu : Translation<T[K]>;
};

type Join<P extends string, K extends string> = P extends '' ? K : `${P}.${K}`;

export type KeyPaths<T, P extends string = ''> = {
  [K in keyof T & string]: T[K] extends string | PluralEn ? Join<P, K> : KeyPaths<T[K], Join<P, K>>;
}[keyof T & string];

export type PluralKeyPaths<T, P extends string = ''> = {
  [K in keyof T & string]: T[K] extends string ? never : T[K] extends PluralEn ? Join<P, K> : PluralKeyPaths<T[K], Join<P, K>>;
}[keyof T & string];

export type Params = Record<string, string | number>;

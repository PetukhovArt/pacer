import StyleDictionary from 'style-dictionary';

const themes = [
  { name: 'light', selector: ':root' },
  { name: 'dark', selector: '.vr-dark' },
] as const;

for (const theme of themes) {
  const sd = new StyleDictionary({
    source: ['tokens/primitive.json', `tokens/semantic.${theme.name}.json`],
    log: { verbosity: 'silent' },
    platforms: {
      css: {
        transformGroup: 'css',
        prefix: 'vr',
        buildPath: 'generated/',
        files: [{ destination: `${theme.name}.css`, format: 'css/variables', options: { selector: theme.selector, outputReferences: false } }],
      },
    },
  });
  await sd.buildAllPlatforms();
}

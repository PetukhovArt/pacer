const OLD_PREFIX = 'veyra.';
const NEW_PREFIX = 'berth.';
const DONE = 'berth.storage.adopted';
const OLD_VIEW = 'veyra.view.v2.';
const NEW_VIEW = 'berth.view.v2';

function adoptLegacyStorage(storage: Storage) {
  if (storage.getItem(DONE) !== null) return;
  const keys = Array.from({ length: storage.length }, (_, index) => storage.key(index)).filter((key): key is string => key?.startsWith(OLD_PREFIX) ?? false);
  for (const key of keys) {
    const target = key.startsWith(OLD_VIEW) ? NEW_VIEW : NEW_PREFIX + key.slice(OLD_PREFIX.length);
    const value = storage.getItem(key);
    if (value !== null && storage.getItem(target) === null) storage.setItem(target, value);
  }
  storage.setItem(DONE, '1');
}

adoptLegacyStorage(localStorage);

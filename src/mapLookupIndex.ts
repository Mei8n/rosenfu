// Keep the first match, just like Array.find, even for imported duplicate keys.
export function firstByKey<T, K>(items: readonly T[], keyOf: (item:T)=>K): Map<K, T> {
  const index = new Map<K, T>();
  for (const item of items) {
    const key = keyOf(item);
    if (!index.has(key)) index.set(key, item);
  }
  return index;
}

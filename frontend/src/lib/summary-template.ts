export function slugifyTemplateId(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 64);
}

export function createUniqueTemplateId(name: string, existingIds: Iterable<string>): string {
  const base = slugifyTemplateId(name);
  if (!base) return '';

  const used = new Set(existingIds);
  if (!used.has(base)) return base;

  for (let suffix = 2; suffix <= 9999; suffix += 1) {
    const suffixText = `-${suffix}`;
    const candidate = `${base.slice(0, 64 - suffixText.length)}${suffixText}`;
    if (!used.has(candidate)) return candidate;
  }

  return '';
}

export function moveArrayItem<T>(items: readonly T[], from: number, to: number): T[] {
  if (from < 0 || from >= items.length || to < 0 || to >= items.length || from === to) {
    return [...items];
  }

  const next = [...items];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}

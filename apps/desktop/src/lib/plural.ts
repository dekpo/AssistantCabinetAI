/**
 * 0 and 1 take the singular form; 2 and above take the plural. The product prefers this over
 * the "(s)" marker, including for zero, which English would usually pluralise.
 */
export function pluralise(count: number, singular: string, plural: string): string {
  return count > 1 ? plural : singular;
}

export function counted(count: number, singular: string, plural: string): string {
  return `${count} ${pluralise(count, singular, plural)}`;
}

export function biographyKey(kind: string, targetKey: string): string {
  return `${kind}\u0000${targetKey}`;
}

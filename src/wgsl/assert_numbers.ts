/** Check a shader literal that also sizes a host resource or enters a CPU calculation. */
export function assertWgslNumbers(name: string, source: string, ...fragments: string[]): void {
  for (const fragment of fragments) {
    if (!source.includes(fragment)) throw new Error(`${name} no longer matches the TS constant in ${fragment}`);
  }
}

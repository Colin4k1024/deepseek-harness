/** Fail clearly when a value an invariant guarantees is missing. */
export function required<T>(value: T | undefined | null): NonNullable<T> {
  if (value === undefined || value === null) throw new Error('A value required by an invariant is missing')
  return value
}

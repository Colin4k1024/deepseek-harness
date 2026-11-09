/** Default deployment and machine fixtures for tests. */
import type { Volatile } from '@deepseek-ai/cordis'
import type { Deployment, Machine } from '../../src/deployment.ts'

/** Field overrides; an explicit undefined removes an optional field. */
type Overrides<T extends object> = { [K in keyof T]?: T[K] | undefined }

/** Apply overrides to a base value, treating an explicit undefined as field removal. */
function apply<T extends object>(base: T, overrides: Overrides<T>): T {
  let result: T & object = { ...base }
  for (const key of Object.keys(overrides) as Array<keyof T>) {
    const value = overrides[key]
    if (value === undefined) {
      const { [key]: _removed, ...rest } = result
      result = rest as T & object
    }
    else result[key] = value
  }
  return result
}

/** Mutable Volatile reference for simulating live configuration updates. */
export const reference = <T>(value: T): Volatile<T> & { set(next: T): void } => {
  let current = value
  return { get: () => current as never, set: (next) => { current = next } }
}
export const testMachine = (overrides: Overrides<Machine> = {}): Machine => apply({
  stateDir: '/tmp/dsh-superagent-test', promptMaxChars: reference(100000), admissionContextMaxPages: reference(10),
  admissionContextTimeoutMs: reference(15000),
}, overrides)
export const testDeployment = (overrides: Overrides<Deployment> = {}): Deployment => apply({
  ...testMachine(), repository: 'acme/widgets',
  github: { cli: '/bin/false', login: 'agent-bot', noreply: '7+agent-bot@users.noreply.github.com' },
  feishu: { cli: '/bin/false', botId: 'ou_bot', appId: 'cli_bot', brand: 'feishu', name: 'Widgets Bot' },
}, overrides)

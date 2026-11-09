import type { UserConfig } from 'tsdown'
import { clientBundle } from '../../client/tsdown.client.ts'

/** One Node loader entry for a Source watcher, external to every declared dependency. */
const watch = (name: string, source: string): UserConfig => ({
  name: `@deepseek-ai/dsh-experimental-superagent/${name}`,
  entry: { [name]: source },
  outDir: 'lib',
  format: ['esm'],
  platform: 'node',
  target: 'es2024',
  fixedExtension: false,
  dts: false,
  clean: false,
  deps: {
    neverBundle: (specifier: string) =>
      specifier === 'proper-lockfile' || specifier === 'zod' || specifier.startsWith('@deepseek-ai/'),
  },
})

/** Build the Host plugin and Source watchers during the Host pass, and the Web panel during the Client pass. */
export default clientBundle(
  '@deepseek-ai/dsh-experimental-superagent',
  ['lib/types/index.js'],
  {
    hostPhase: true,
    companions: [
      watch('github-watch', 'lib/types/sources/github.js'),
      watch('feishu-watch', 'lib/types/sources/feishu.js'),
    ],
  },
)

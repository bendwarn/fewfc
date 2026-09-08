import { serverNames, stopServer } from './managed-server'

for (const name of serverNames) {
  console.log(`${name}: ${await stopServer(name) ? 'stopped' : 'not running'}`)
}

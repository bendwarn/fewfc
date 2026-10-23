import { expect, test } from 'bun:test'
import { readFile, stat } from 'node:fs/promises'
import { maintenanceBlocksPlayerMutation, maintenanceEnabled } from './maintenance'

test('maintenance is explicit and fails closed for Commands and Replay creation only', () => {
  expect(maintenanceEnabled(undefined)).toBe(false)
  expect(maintenanceEnabled('false')).toBe(false)
  expect(maintenanceEnabled('true')).toBe(true)
  expect(maintenanceBlocksPlayerMutation('POST', '/api/games/room-1/commands')).toBe(true)
  expect(maintenanceBlocksPlayerMutation('POST', '/api/replays')).toBe(true)
  expect(maintenanceBlocksPlayerMutation('GET', '/api/games/room-1/commands')).toBe(false)
  expect(maintenanceBlocksPlayerMutation('POST', '/api/games/room-1/start')).toBe(false)
})

test('deployment preserves the maintenance variable while cutover uses local Wrangler', async () => {
  const workflow = await readFile(new URL('../../../.github/workflows/ci-cd.yml', import.meta.url), 'utf8')
  expect(workflow).toContain('wrangler deploy --env staging --keep-vars')
  expect(workflow).toContain('wrangler deploy --env production --keep-vars')
  expect(workflow).toContain("if: github.event_name == 'push' && github.ref == 'refs/heads/main'")
  expect(workflow).toContain("if: github.event_name == 'workflow_dispatch' && github.ref == 'refs/heads/main' && inputs.deploy_production == true")
  const stagingJob = workflow.slice(workflow.indexOf('  deploy-staging:'), workflow.indexOf('  deploy-production:'))
  expect(stagingJob).toContain("if: github.event_name == 'push' && github.ref == 'refs/heads/main'")
  expect(stagingJob).not.toContain('workflow_dispatch')
  expect(workflow).not.toContain('MAINTENANCE_MODE:false')
  expect(workflow).not.toContain('versions upload')

  const wrangler = await readFile(new URL('../wrangler.toml', import.meta.url), 'utf8')
  expect(wrangler).not.toContain('MAINTENANCE_MODE')

  const maintenanceWorkflowExists = await stat(
    new URL('../../../.github/workflows/maintenance-mode.yml', import.meta.url),
  ).then(() => true, () => false)
  expect(maintenanceWorkflowExists).toBe(false)
})

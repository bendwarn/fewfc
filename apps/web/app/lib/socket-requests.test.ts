import { expect, test } from 'bun:test'
import { createSocketRequests } from './socket-requests'

test('out-of-order replies settle only their own requests, including a query before a command', async () => {
  const sent: { requestId: string; operation: string }[] = []
  const client = createSocketRequests(wire => { sent.push(JSON.parse(wire)); return true })
  let commandFinished = false
  const command = client.request('command', {}).then(value => { commandFinished = true; return value })
  const query = client.request('query', {})
  client.receive({ type: 'result', requestId: sent[1]!.requestId, ok: true, data: ['choice'] })
  expect(await query).toEqual(['choice'])
  expect(commandFinished).toBe(false)
  client.receive({ type: 'result', requestId: sent[0]!.requestId, ok: true, data: { pendingChoice: 'discard' } })
  expect(await command).toEqual({ pendingChoice: 'discard' })
})

test('disconnect releases pending requests and late results cannot complete a newer request', async () => {
  const sent: { requestId: string }[] = []
  const client = createSocketRequests(wire => { sent.push(JSON.parse(wire)); return true })
  const first = client.request('command', {}).catch(error => error.data.code)
  client.cancel()
  expect(await first).toBe('socketDisconnected')
  let finished = false
  const second = client.request('command', {}).then(value => { finished = true; return value })
  client.receive({ type: 'result', requestId: sent[0]!.requestId, ok: true, data: 'old' })
  await Promise.resolve()
  expect(finished).toBe(false)
  client.receive({ type: 'result', requestId: sent[1]!.requestId, ok: true, data: 'new' })
  expect(await second).toBe('new')
  expect(sent).toHaveLength(2)
})

test('failed sends, timeout, cancellation and server errors all settle without buffering commands', async () => {
  await expect(createSocketRequests(() => false).request('command', {})).rejects.toMatchObject({ data: { code: 'socketDisconnected' } })
  await expect(createSocketRequests(() => true, 5).request('command', {})).rejects.toMatchObject({ data: { code: 'socketTimeout' } })
  let id = ''
  const client = createSocketRequests(wire => { id = JSON.parse(wire).requestId; return true })
  const controller = new AbortController()
  const canceled = client.request('ready', true, controller.signal)
  controller.abort()
  await expect(canceled).rejects.toMatchObject({ name: 'AbortError' })
  const denied = client.request('command', {})
  client.receive({ type: 'result', requestId: id, ok: false, error: { statusCode: 403, code: 'notMember', message: '無法操作此房間。' } })
  await expect(denied).rejects.toMatchObject({ statusCode: 403, data: { code: 'notMember' } })
})

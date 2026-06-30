import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { roomRouteResult, safeInternalPath } from './navigation'

describe('safeInternalPath', () => {
  test('accepts only application routes', () => {
    assert.equal(safeInternalPath('/rooms/abc?invite=secret'), '/rooms/abc?invite=secret')
    assert.equal(safeInternalPath('/rooms?tab=join'), '/rooms?tab=join')
    assert.equal(safeInternalPath('/deck'), '/deck')
    assert.equal(safeInternalPath('/login'), undefined)
  })

  test('rejects external and protocol-relative redirects', () => {
    assert.equal(safeInternalPath('https://example.com/rooms'), undefined)
    assert.equal(safeInternalPath('//example.com/rooms'), undefined)
    assert.equal(safeInternalPath(undefined), undefined)
  })
})

describe('roomRouteResult', () => {
  test('maps room failures to player-facing results', () => {
    assert.equal(roomRouteResult(new Error('room is full')), '房間已滿')
    assert.equal(roomRouteResult(new Error('room has already started')), '對局已開始')
    assert.equal(roomRouteResult(new Error('room not found')), '找不到這個房間')
  })
})

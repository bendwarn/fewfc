import { expect, test } from 'bun:test'
import { selectJoinablePublicRooms } from './lobby-room-selection'

test('excludes rooms already joined as a player or observer from public discovery', () => {
  const rooms = [
    { gameId: 'joined-as-player', isMember: true, isObserver: false },
    { gameId: 'joined-as-observer', isMember: false, isObserver: true },
    { gameId: 'available', isMember: false, isObserver: false },
  ]

  expect(selectJoinablePublicRooms(rooms).map(room => room.gameId)).toEqual(['available'])
})

test('shows a public room again after the observer leaves', () => {
  const room = { gameId: 'observed-room', isMember: false, isObserver: true }

  expect(selectJoinablePublicRooms([room])).toEqual([])
  expect(selectJoinablePublicRooms([{ ...room, isObserver: false }])).toEqual([
    { ...room, isObserver: false },
  ])
})

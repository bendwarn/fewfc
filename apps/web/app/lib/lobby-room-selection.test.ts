import { expect, test } from 'bun:test'
import { selectJoinablePublicRooms } from './lobby-room-selection'

test('excludes rooms already joined as a player or observer from public discovery', () => {
  const rooms = [
    { gameId: 'joined-as-player', members: [{ userId: 'guest-user' }], observers: [] },
    { gameId: 'joined-as-observer', members: [{ userId: 'host-user' }], observers: [{ userId: 'guest-user' }] },
    { gameId: 'available', members: [{ userId: 'host-user' }], observers: [{ userId: 'another-user' }] },
  ]

  expect(selectJoinablePublicRooms(rooms, 'guest-user').map(room => room.gameId)).toEqual(['available'])
  expect(selectJoinablePublicRooms(rooms, 'outsider-user').map(room => room.gameId)).toEqual([
    'joined-as-player',
    'joined-as-observer',
    'available',
  ])
})

test('shows a public room again after the observer leaves', () => {
  const room = { gameId: 'observed-room', members: [{ userId: 'host-user' }], observers: [{ userId: 'guest-user' }] }

  expect(selectJoinablePublicRooms([room], 'guest-user')).toEqual([])
  expect(selectJoinablePublicRooms([{ ...room, observers: [] }], 'guest-user')).toEqual([
    { ...room, observers: [] },
  ])
})

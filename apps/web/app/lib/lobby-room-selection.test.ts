import { expect, test } from 'bun:test'
import { selectJoinablePublicRooms } from './lobby-room-selection'

test('excludes rooms already joined by the current player from public discovery', () => {
  const rooms = [
    { gameId: 'joined', members: [{ userId: 'guest-user' }] },
    { gameId: 'available', members: [{ userId: 'host-user' }] },
  ]

  expect(selectJoinablePublicRooms(rooms, 'guest-user').map(room => room.gameId)).toEqual(['available'])
})

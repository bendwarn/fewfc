import type { GameRoomLobbyItem, GameRoomLobbyResponse } from '../../shared/game-room'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const rooms = await listPublicRooms(event)
  const myRooms = await listPlayerRooms(event, session.user.id)
  const lobbyItem = (room: typeof rooms[number]): GameRoomLobbyItem => ({
    gameId: room.gameId,
    roomCode: room.roomCode,
    name: room.name,
    status: room.status,
    memberCount: room.members.length,
    observerCount: room.observers.length,
    capacity: room.capacity,
    ruleVersion: room.ruleVersion,
    enabledRuleModules: room.enabledRuleModules,
    isMember: room.members.some(member => member.userId === session.user.id),
    isObserver: room.observers.some(observer => observer.userId === session.user.id),
  })

  return {
    rooms: rooms.map(lobbyItem),
    myRooms: myRooms.map(lobbyItem),
  } satisfies GameRoomLobbyResponse
})

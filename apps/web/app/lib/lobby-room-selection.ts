interface PublicRoomWithMembers {
  isMember: boolean
  isObserver: boolean
}

export function selectJoinablePublicRooms<Room extends PublicRoomWithMembers>(
  rooms: readonly Room[],
): Room[] {
  return rooms.filter(room => !room.isMember && !room.isObserver)
}

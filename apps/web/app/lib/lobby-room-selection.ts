interface PublicRoomMembership {
  userId: string
}

interface PublicRoomWithMembers {
  members: readonly PublicRoomMembership[]
  observers: readonly PublicRoomMembership[]
}

export function selectJoinablePublicRooms<Room extends PublicRoomWithMembers>(
  rooms: readonly Room[],
  userId: string,
): Room[] {
  return rooms.filter(room => (
    !room.members.some(member => member.userId === userId)
    && !room.observers.some(observer => observer.userId === userId)
  ))
}

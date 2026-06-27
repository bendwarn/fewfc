export default defineEventHandler(async (event) => {
  const session = await requireSession(event)

  return {
    rooms: await listPublicRooms(event),
    myRooms: await listPlayerRooms(event, session.user.id),
  }
})

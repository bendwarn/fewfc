export default defineEventHandler(async (event) => {
  await requireSession(event)

  return {
    rooms: await listPublicRooms(event),
  }
})

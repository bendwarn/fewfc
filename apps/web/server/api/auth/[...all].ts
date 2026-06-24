export default defineEventHandler(async (event) => {
  return authForEvent(event).handler(toWebRequest(event))
})

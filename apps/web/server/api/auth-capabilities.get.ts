import { socialProviderCapabilities } from '../utils/social-auth'
import { workerEnv } from '../utils/worker-env'

export default defineEventHandler((event) => ({
  providers: socialProviderCapabilities(workerEnv(event)),
}))

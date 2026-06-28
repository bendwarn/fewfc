import tailwindcss from '@tailwindcss/vite'

const appEnvironment = process.env.APP_ENV

if (!['development', 'staging', 'production'].includes(appEnvironment ?? '')) {
  throw new Error('APP_ENV must be development, staging, or production.')
}

// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  compatibilityDate: '2025-07-15',
  css: ['~/assets/css/main.css'],
  devtools: { enabled: appEnvironment === 'development' },
  runtimeConfig: {
    public: {
      appEnv: appEnvironment,
    },
  },
  vite: {
    plugins: [tailwindcss()],
  },
  typescript: {
    tsConfig: {
      compilerOptions: {
        types: ['node', '@cloudflare/workers-types'],
      },
    },
  },
  nitro: {
    preset: 'cloudflare-module',
  },
})

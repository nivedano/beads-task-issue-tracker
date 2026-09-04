import tailwindcss from '@tailwindcss/vite'
import pkg from './package.json'

const isDevMode = process.env.NODE_ENV !== 'production'

export default defineNuxtConfig({
  runtimeConfig: {
    public: {
      appVersion: pkg.version,
    },
  },
  compatibilityDate: '2025-01-01',
  future: {
    compatibilityVersion: 4,
  },

  ssr: false,

  nitro: {
    storage: {
      // Avoid Nuxt's file-URL prerender cache driver, which Rollup cannot
      // resolve on Windows. Release generation only needs this cache in-process.
      'internal:nuxt:prerender': {
        driver: 'memory',
      },
    },
  },

  router: {
    options: {
      hashMode: true,
    },
  },

  app: {
    cdnURL: './',
  },

  devServer: {
    port: 3133,
  },

  modules: ['shadcn-nuxt'],

  shadcn: {
    prefix: '',
    componentDir: './app/components/ui',
  },

  vite: {
    plugins: [tailwindcss()],
    esbuild: {
      pure: ['console.log', 'console.debug', 'console.info', 'console.warn'],
      legalComments: 'none',
    },
    build: {
      sourcemap: isDevMode,
      target: 'es2020',
      minify: 'esbuild',
    },
  },

  css: ['~/assets/css/tailwind.css'],

  devtools: { enabled: false },
})

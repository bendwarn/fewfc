<template>
  <NuxtRouteAnnouncer />
  <NuxtLayout :name="layoutName">
    <NuxtPage />
  </NuxtLayout>
</template>

<script setup lang="ts">
import {
  canonicalUrl,
  pageTitle,
  robotsDirective,
  SITE_DESCRIPTION,
  SITE_NAME,
  SOCIAL_IMAGE_URL,
  socialPageUrl,
  STRUCTURED_DATA,
} from '~/lib/site-metadata'

const route = useRoute()
const runtimeConfig = useRuntimeConfig()
const layoutName = computed(() => typeof route.meta.layout === 'string' ? route.meta.layout : 'default')
const title = computed(() => pageTitle(route.path))
const canonical = computed(() => canonicalUrl(route.path))
const robots = computed(() => robotsDirective(route.path, runtimeConfig.public.appEnv))
const socialUrl = computed(() => socialPageUrl(route.path))
const structuredData = JSON.stringify(STRUCTURED_DATA).replaceAll('<', '\\u003c')

useHead(() => ({
  title: title.value,
  link: canonical.value ? [{ rel: 'canonical', href: canonical.value }] : [],
  script: route.path === '/'
    ? [{ key: 'site-structured-data', type: 'application/ld+json', innerHTML: structuredData }]
    : [],
}))

useSeoMeta({
  description: SITE_DESCRIPTION,
  robots,
  ogType: 'website',
  ogLocale: 'zh_TW',
  ogSiteName: SITE_NAME,
  ogTitle: SITE_NAME,
  ogDescription: SITE_DESCRIPTION,
  ogImage: SOCIAL_IMAGE_URL,
  ogImageAlt: '五行戰鬥牌—以牌為陣，決勝五行',
  ogImageWidth: 1200,
  ogImageHeight: 630,
  ogUrl: socialUrl,
  twitterCard: 'summary_large_image',
  twitterTitle: SITE_NAME,
  twitterDescription: SITE_DESCRIPTION,
  twitterImage: SOCIAL_IMAGE_URL,
  twitterImageAlt: '五行戰鬥牌—以牌為陣，決勝五行',
})
</script>

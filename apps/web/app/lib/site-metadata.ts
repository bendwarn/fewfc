export const SITE_NAME = '五行戰鬥牌'
export const SITE_DESCRIPTION = '運用金、木、水、火、土的生剋關係組合陣法，與其他玩家在線交鋒，在每一次出牌中掌握戰局。'
export const SITE_ORIGIN = 'https://fewfc.febcg.workers.dev'
export const SITE_URL = `${SITE_ORIGIN}/`
export const SOCIAL_IMAGE_URL = `${SITE_ORIGIN}/og-image.png`
export const OFFICIAL_SITE_URL = 'https://www.cfecards.org/'

function pageName(path: string): string | null {
  if (path === '/login') return '登入'
  if (path === '/reset-password') return '重設密碼'
  if (path === '/rooms') return '對戰大廳'
  if (path.startsWith('/rooms/')) return '對戰房間'
  if (path === '/deck') return '個人牌組'
  if (path === '/replays') return '重播紀錄'
  if (path.startsWith('/replays/')) return '對戰重播'
  return null
}

export function pageTitle(path: string): string {
  const name = pageName(path)
  return name ? `${name}｜${SITE_NAME}` : SITE_NAME
}

export function robotsDirective(path: string, appEnvironment: string | undefined): string {
  return appEnvironment === 'production' && path === '/'
    ? 'index, follow'
    : 'noindex, nofollow'
}

export function canonicalUrl(path: string): string | undefined {
  return path === '/' ? SITE_URL : undefined
}

export function socialPageUrl(path: string): string {
  return new URL(path, SITE_URL).toString()
}

export const STRUCTURED_DATA = {
  '@context': 'https://schema.org',
  '@graph': [
    {
      '@type': 'WebSite',
      '@id': `${SITE_URL}#website`,
      name: SITE_NAME,
      url: SITE_URL,
      description: SITE_DESCRIPTION,
      inLanguage: 'zh-Hant',
      mainEntity: { '@id': `${SITE_URL}#game` },
    },
    {
      '@type': 'VideoGame',
      '@id': `${SITE_URL}#game`,
      name: SITE_NAME,
      url: SITE_URL,
      image: SOCIAL_IMAGE_URL,
      description: SITE_DESCRIPTION,
      inLanguage: 'zh-Hant',
      genre: ['卡牌遊戲', '策略遊戲'],
      gamePlatform: 'Web browser',
      playMode: 'MultiPlayer',
      numberOfPlayers: {
        '@type': 'QuantitativeValue',
        minValue: 2,
        maxValue: 4,
      },
      isPartOf: { '@id': `${SITE_URL}#website` },
    },
  ],
} as const

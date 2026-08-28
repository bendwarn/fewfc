import { describe, expect, test } from 'bun:test'
import {
  canonicalUrl,
  pageTitle,
  robotsDirective,
  SITE_NAME,
  SITE_URL,
  socialPageUrl,
  STRUCTURED_DATA,
} from './site-metadata'

describe('site metadata', () => {
  test('uses the brand alone on the homepage and names application routes', () => {
    expect(pageTitle('/')).toBe(SITE_NAME)
    expect(pageTitle('/rooms')).toBe(`對戰大廳｜${SITE_NAME}`)
    expect(pageTitle('/privacy')).toBe(`隱私權政策｜${SITE_NAME}`)
    expect(pageTitle('/rooms/room-7')).toBe(`對戰房間｜${SITE_NAME}`)
    expect(pageTitle('/replays/replay-9')).toBe(`對戰重播｜${SITE_NAME}`)
  })

  test('only indexes the production homepage', () => {
    expect(robotsDirective('/', 'production')).toBe('index, follow')
    expect(robotsDirective('/', 'staging')).toBe('noindex, nofollow')
    expect(robotsDirective('/privacy', 'production')).toBe('index, follow')
    expect(robotsDirective('/rooms', 'production')).toBe('noindex, nofollow')
  })

  test('publishes one homepage canonical and absolute social URLs', () => {
    expect(canonicalUrl('/')).toBe(SITE_URL)
    expect(canonicalUrl('/privacy')).toBe(`${SITE_URL}privacy`)
    expect(canonicalUrl('/rooms')).toBeUndefined()
    expect(socialPageUrl('/rooms/room-7')).toBe(`${SITE_URL}rooms/room-7`)
  })

  test('describes both the website and multiplayer browser game', () => {
    expect(STRUCTURED_DATA['@graph'].map(entry => entry['@type'])).toEqual(['WebSite', 'VideoGame'])
    expect(STRUCTURED_DATA['@graph'][1]).toMatchObject({
      gamePlatform: 'Web browser',
      playMode: 'MultiPlayer',
    })
  })
})

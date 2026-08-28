import { describe, expect, test } from 'bun:test'
import {
  activeAnonymousUpgradeMessage,
  anonymousMigrationStrategy,
  canAnonymousUpgrade,
  canUnlinkAccount,
  providerUserUpdateWithCanonicalName,
  shouldFillAvatar,
  socialAuthConfiguration,
  socialCredentials,
  socialProviderCapabilities,
} from './social-auth'

describe('social authentication policy', () => {
  test('only exposes providers with a complete credential pair', () => {
    expect(socialProviderCapabilities({
      GOOGLE_CLIENT_ID: 'google-id',
      GOOGLE_CLIENT_SECRET: 'google-secret',
      GITHUB_CLIENT_ID: 'github-id',
    })).toEqual({ google: true, github: false })
    expect(socialCredentials({ GOOGLE_CLIENT_ID: '  ', GOOGLE_CLIENT_SECRET: 'secret' })).toEqual({})
  })

  test('uses encrypted OAuth tokens and requires explicit different-email linking', () => {
    const configuration = socialAuthConfiguration({
      GOOGLE_CLIENT_ID: 'google-id',
      GOOGLE_CLIENT_SECRET: 'google-secret',
    })
    expect(configuration.socialProviders).toEqual({
      google: { clientId: 'google-id', clientSecret: 'google-secret' },
    })
    expect(configuration.account).toMatchObject({
      encryptOAuthTokens: true,
      accountLinking: {
        disableImplicitLinking: true,
        allowDifferentEmails: true,
      },
    })
  })

  test('only fills a missing Player Profile avatar from a provider', () => {
    expect(shouldFillAvatar(null, 'provider.png')).toBe(true)
    expect(shouldFillAvatar('', 'provider.png')).toBe(true)
    expect(shouldFillAvatar('game-avatar.png', 'provider.png')).toBe(false)
    expect(shouldFillAvatar(null, null)).toBe(false)
  })

  test('preserves the canonical user name during an explicit provider link', () => {
    expect(providerUserUpdateWithCanonicalName<{ name?: string, image?: string }>({ name: 'Google 顯示名稱', image: 'provider.png' })).toEqual({
      name: undefined,
      image: 'provider.png',
    })
  })

  test('requires another login method before unlinking', () => {
    expect(canUnlinkAccount([{ id: 'credential', providerId: 'credential', createdAt: new Date() }])).toBe(false)
    expect(canUnlinkAccount([
      { id: 'credential', providerId: 'credential', createdAt: new Date() },
      { id: 'google', providerId: 'google', createdAt: new Date() },
    ])).toBe(true)
  })

  test('blocks anonymous upgrades while a waiting or active room exists', () => {
    expect(canAnonymousUpgrade(true)).toBe(false)
    expect(canAnonymousUpgrade(false)).toBe(true)
    expect(activeAnonymousUpgradeMessage).toContain('離開')
  })

  test('preserves the established account on an anonymous merge conflict', () => {
    expect(anonymousMigrationStrategy({
      anonymousCreatedAt: 200,
      targetCreatedAt: 100,
      anonymousAvatarUrl: 'guest.png',
      targetAvatarUrl: 'member.png',
    })).toMatchObject({
      targetIsNew: false,
      copyAnonymousName: false,
      copyAnonymousDeckWhenTargetMissing: true,
      preserveTargetReplayOnConflict: true,
      copyAnonymousAvatar: false,
    })
  })

  test('lets a newly-created social account retain anonymous identity data', () => {
    expect(anonymousMigrationStrategy({
      anonymousCreatedAt: 100,
      targetCreatedAt: 200,
      anonymousAvatarUrl: null,
      targetAvatarUrl: 'provider.png',
    })).toMatchObject({
      targetIsNew: true,
      copyAnonymousName: true,
      copyAnonymousDeckWhenTargetMissing: true,
      preserveTargetReplayOnConflict: true,
    })
  })
})

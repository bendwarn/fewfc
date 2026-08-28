import { describe, expect, test } from 'bun:test'
import { resolveAuthNavigation } from './auth-navigation'

describe('authentication navigation policy', () => {
  test('shows the public homepage while signed out and keeps the signed-in shortcut to rooms', () => {
    expect(resolveAuthNavigation({
      authenticated: false,
      path: '/',
      fullPath: '/',
    })).toEqual({ allow: true })

    expect(resolveAuthNavigation({
      authenticated: true,
      path: '/',
      fullPath: '/',
    })).toEqual({ redirect: '/rooms' })
  })

  test('sends an unauthenticated room deep link to login and retains its invite', () => {
    expect(resolveAuthNavigation({
      authenticated: false,
      path: '/rooms/room-7',
      fullPath: '/rooms/room-7?invite=invite-token',
    })).toEqual({ redirect: '/login?redirect=%2Frooms%2Froom-7%3Finvite%3Dinvite-token' })
  })

  test('keeps the privacy policy public while signed out', () => {
    expect(resolveAuthNavigation({
      authenticated: false,
      path: '/privacy',
      fullPath: '/privacy',
    })).toEqual({ allow: true })

    expect(resolveAuthNavigation({
      authenticated: true,
      path: '/privacy',
      fullPath: '/privacy',
    })).toEqual({ allow: true })
  })

  test('retains replay deep links and query semantics through authentication', () => {
    expect(resolveAuthNavigation({
      authenticated: false,
      path: '/replays/replay-9',
      fullPath: '/replays/replay-9?step=12',
    })).toEqual({ redirect: '/login?redirect=%2Freplays%2Freplay-9%3Fstep%3D12' })

    expect(resolveAuthNavigation({
      authenticated: true,
      path: '/login',
      fullPath: '/login?redirect=%2Freplays%2Freplay-9%3Fstep%3D12',
      redirect: '/replays/replay-9?step=12',
    })).toEqual({ redirect: '/replays/replay-9?step=12' })
  })

  test('sends an unknown protected route to login', () => {
    expect(resolveAuthNavigation({
      authenticated: false,
      path: '/missing',
      fullPath: '/missing',
    })).toEqual({ redirect: '/login' })
  })
})

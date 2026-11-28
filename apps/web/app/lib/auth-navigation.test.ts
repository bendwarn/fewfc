import { describe, expect, test } from 'bun:test'
import { resolveAuthNavigation } from './auth-navigation'

describe('authentication navigation policy', () => {
  test('shows the public homepage for visitors with or without a session', () => {
    expect(resolveAuthNavigation({
      authenticated: false,
      path: '/',
      fullPath: '/',
    })).toEqual({ allow: true })

    expect(resolveAuthNavigation({
      authenticated: true,
      path: '/',
      fullPath: '/?redirect=/rooms',
      redirect: '/rooms',
    })).toEqual({ allow: true })
  })

  test('preserves the lobby as the destination when a player needs to sign in', () => {
    expect(resolveAuthNavigation({
      authenticated: false,
      path: '/rooms',
      fullPath: '/rooms',
    })).toEqual({ redirect: '/login?redirect=%2Frooms' })

    expect(resolveAuthNavigation({
      authenticated: true,
      path: '/login',
      fullPath: '/login?redirect=%2Frooms',
      redirect: '/rooms',
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

  test('allows competition replay pages anonymously while ordinary saved replays remain private', () => {
    expect(resolveAuthNavigation({
      authenticated: false,
      path: '/replays/competition%3Aevent-1%3Agame-1',
      fullPath: '/replays/competition%3Aevent-1%3Agame-1?step=12',
    })).toEqual({ allow: true })

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

  test('allows competition result pages while signed out', () => {
    expect(resolveAuthNavigation({
      authenticated: false,
      path: '/competitions/event-1',
      fullPath: '/competitions/event-1',
    })).toEqual({ allow: true })
  })

  test('preserves the competition index through login and accepts a competition detail as a return target', () => {
    expect(resolveAuthNavigation({
      authenticated: false,
      path: '/competitions',
      fullPath: '/competitions',
    })).toEqual({ redirect: '/login?redirect=%2Fcompetitions' })

    expect(resolveAuthNavigation({
      authenticated: true,
      path: '/login',
      fullPath: '/login?redirect=%2Fcompetitions%2Fevent-1',
      redirect: '/competitions/event-1',
    })).toEqual({ redirect: '/competitions/event-1' })
  })

  test('sends an unknown protected route to login', () => {
    expect(resolveAuthNavigation({
      authenticated: false,
      path: '/missing',
      fullPath: '/missing',
    })).toEqual({ redirect: '/login' })
  })
})

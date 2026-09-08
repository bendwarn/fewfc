import { expect, test } from 'bun:test'
import { presentActionDetail } from './action-detail-presentation'
import { cardChoiceAnswer, declineChoiceAnswer } from './pending-choice-interaction'
import { presentPendingChoice } from './pending-choice-presentation'
import { presentTotem } from './totem-presentation'
import type { FormationEffect, PendingChoice } from '../types/fewfc'

test('Totem descriptions preserve Shield exclusions, mandatory consumption and Sacred Beast exception', () => {
  for (const totem of ['AzureHorn', 'WhiteFang', 'VermilionFeather', 'BlackShell', 'YellowScales'] as const) {
    expect(presentTotem(totem)).toContain('防護罩不適用')
    expect(presentTotem(totem)).toContain('自動捨棄圖騰')
    expect(presentTotem(totem)).toContain('聖獸無視圖騰效果')
  }
  expect(presentTotem('YellowScales')).toContain('幻化、混沌')
  expect(presentTotem('AzureHorn')).toContain('青角圖騰')
})

test('Totem action effects explain ordered resolution and mandatory selection', () => {
  const present = (effect: FormationEffect) => presentActionDetail({ consequences: [{ type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'resolveFormationEffect', effect } }] })
  expect(present({ type: 'totemVermilionFeather' })).toContain('先結算攻擊，再轉換為火行環境')
  expect(present({ type: 'totemYellowScales' })).toContain('若非空手，必須選擇一張')
  expect(present({ type: 'dragonSearch' })).toContain('公開展示')
  expect(present({ type: 'dragonSearch' })).toContain('亦可放棄並洗牌')
  expect(present({ type: 'totemAzureHorn' })).toContain('15 點防護罩')
  expect(present({ type: 'totemWhiteFang' })).toContain('30 點生命')
  expect(present({ type: 'totemBlackShell' })).toContain('下家下回合無法行動及抽牌')
})

test('Yellow hand theft requires a card while Dragon search may decline', () => {
  const choice: PendingChoice = { type: 'card', cards: [], minimum: 1, maximum: 1, canDecline: false }
  expect(cardChoiceAnswer(choice, [])).toBeUndefined()
  expect(cardChoiceAnswer(choice, [5])).toEqual({ type: 'cards', cards: [5] })
  expect(choice.canDecline).toBeFalse()
  expect(declineChoiceAnswer()).toEqual({ type: 'decline' })
  expect(presentPendingChoice({ type: 'centralSpiritArrayCard' })).toContain('必須選擇')
  expect(presentPendingChoice({ type: 'dragonSearchDeckCard' })).toContain('公開展示')
  expect(presentPendingChoice({ type: 'southSpiritArrayElement' })).toContain('攻擊的五行屬性')
})

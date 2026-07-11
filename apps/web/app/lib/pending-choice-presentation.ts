import type { PendingChoicePresentation } from '../types/fewfc'
import { echoMelodyLabels } from './echo-presentation'

export function presentPendingChoice(presentation: PendingChoicePresentation): string {
  switch (presentation.type) {
    case 'turnDrawDiscard': return '選擇一張本回合抽到的牌捨棄'
    case 'holyWind': return '聖風：選擇一張最高等級牌加入手牌'
    case 'chaos': return '混沌：選擇兩張牌放回目標玩家手牌'
    case 'revelation': return '啟示：從抽出的三張牌中選擇一張'
    case 'azureCloudStep': return '蒼雲步：選擇一張抽到的牌放回牌組'
    case 'clearWindTenThousandMiles': return '清風萬里：選擇要保留的牌'
    case 'mirrorResonance': return '鏡鳴：選擇對方一張手牌捨棄'
    case 'myriadResonance': return '萬鳴：選擇對方一張手牌捨棄'
    case 'thousandResonance': return '千鳴：選擇對方一張手牌捨棄'
    case 'echoRingingMetalDeckCard': return '商調‧鳴金：從牌組選擇一張牌'
    case 'echoCost': return `${echoMelodyLabels[presentation.melody]}：選擇一張手牌支付迴響代價，或放棄迴響`
    case 'echoSplitEarthFormation': return '宮調‧裂土：選擇要壓制的陣法'
    case 'echoPureFirePlayer': return '變徵‧淨火：選擇受影響玩家'
    case 'echoPlantEarthMelody': return '變宮‧植土：選擇要執行的曲調主效果'
    case 'earthRendingEnvironment': return '裂地崩山：選擇要轉移的環境'
    case 'earthRendingCard': return '裂地崩山：選擇一張環行牌捨棄或公開'
    case 'metamorphosis': return '幻化：選擇效果指定的牌'
    case 'sealCard': return '選擇要封印的牌'
    case 'unclassified': return '等待選擇'
  }
  const exhaustive: never = presentation
  return exhaustive
}

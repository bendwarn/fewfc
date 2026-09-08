import type { TotemKind } from '../types/fewfc'

export const totemLabels: Record<TotemKind, string> = {
  AzureHorn: '青角圖騰',
  WhiteFang: '白牙圖騰',
  VermilionFeather: '朱羽圖騰',
  BlackShell: '玄甲圖騰',
  YellowScales: '黃鱗圖騰',
}

const totemRules: Record<TotemKind, { element: string; formations: string }> = {
  AzureHorn: { element: '木', formations: '防禦、氣壁' },
  WhiteFang: { element: '金', formations: '武器、光芒' },
  VermilionFeather: { element: '火', formations: '反震、震暴' },
  BlackShell: { element: '水', formations: '封印、歸元' },
  YellowScales: { element: '土', formations: '幻化、混沌' },
}

export function presentTotem(totem: TotemKind): string {
  const { element, formations } = totemRules[totem]
  return `${totemLabels[totem]}：自身承受${element}行傷害時不因環境加倍（防護罩不適用）；施展${formations}不因環境無效；自身${element}行攻擊因環境將改為恢復生命時，自動捨棄圖騰並阻止該轉換。聖獸無視圖騰效果。獲得新圖騰時取代舊圖騰；虛空斷脈術成功破除環境時破除所有圖騰。`
}

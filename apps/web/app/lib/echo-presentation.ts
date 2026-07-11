export type EchoMelodyPresentation =
  | 'ringingMetal'
  | 'fallingWood'
  | 'flowingWater'
  | 'warFire'
  | 'splitEarth'
  | 'pureFire'
  | 'unclassified'

export const echoMelodyLabels: Record<EchoMelodyPresentation, string> = {
  ringingMetal: '商調‧鳴金',
  fallingWood: '角調‧落木',
  flowingWater: '羽調‧流水',
  warFire: '徵調‧戰火',
  splitEarth: '宮調‧裂土',
  pureFire: '變徵‧淨火',
  unclassified: '曲調',
}

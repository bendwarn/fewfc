# Distinguish Jianghu states from generic status effects

Keep the established `StatusEffect` model and serialized `statuses` contract
unchanged as the rules engine's generic representation for existing ongoing
effects. Introduce a separate typed **Jianghu State (江湖狀態)** model containing
only 千鋒, 踏雪, and 中毒, including the source and duration data their published
rules require. This preserves replay and Web API compatibility while preventing
the Jianghu rulebook term from being incorrectly applied to every ongoing
effect.

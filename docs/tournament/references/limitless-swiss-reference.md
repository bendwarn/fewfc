# Limitless 瑞士制行為參考

查證：2026-09-13。官方文件與公開樣本，非開源核心還原。本站積分／晉級政策只見[設計](../competitive-play-design.md)。

## API 與文件能力

當時官方文件／GitHub 搜尋未找到平台或配對核心公開原碼；第三方 wrapper 不算平台實作。[Developer](https://docs.limitlesstcg.com/developer.html)公開查詢與 ended webhook，[Tournament API](https://docs.limitlesstcg.com/developer/tournaments)列 details／standings／pairings，沒有配對管理寫入契約。

[Reference](https://docs.limitlesstcg.com/organizer/reference)說明 Swiss 輪數由主辦設定、同分使用大致參考 Pokémon 的對手勝率、完全同分最終隨機排序；不能據此確定公式或求解器。[Procedures](https://docs.limitlesstcg.com/organizer/procedures)分開封存本輪與產生下輪，亦可刪當輪重配。

## 一個公開樣本

Torneo Online Alain Cards #10 Edición Especial，2026-09-12，9 人：

- [details](https://play.limitlesstcg.com/api/tournaments/6aa1ff8dab080c8c957ffafe/details)：四輪 Swiss BO1，接兩輪 SE BO3。
- [pairings](https://play.limitlesstcg.com/api/tournaments/6aa1ff8dab080c8c957ffafe/pairings)：前三輪三名不同輪空者，輪空有勝方但缺第二參賽者；第二輪含跨戰績配對，Swiss 未見重複對手。
- [standings](https://play.limitlesstcg.com/api/tournaments/6aa1ff8dab080c8c957ffafe/standings)：一人 drop=3，第四輪四場完整交手、無輪空；Swiss 對手可於淘汰再遇。

只支持上述樣本觀察，不證全域最佳化、輪空優先序、隨機性或所有例外。

## 公式仍不能從來源補齊

當時[官方手冊 URL](https://www.pokemon.com/static-assets/content-assets/cms2/pdf/play-pokemon/rules/play-pokemon-tournament-rules-handbook-en.pdf)返回驗證 HTML，未繞過。[2011 官方程序](https://assets.pokemon.com/assets/cms/pdf/op/tournaments/2011/Pokemon_Tournament_Operation_Procedures.pdf)的勝率／退賽條款不能代表現行 Limitless；第三方對和局折算不一致，未採為公式。

## 2026-10-10 積分設定補查

[官方 Point Structure](https://docs.limitlesstcg.com/organizer/reference.html#point-structure)區分按交手結果與按交手內各局計分，另有全勝加分選項；和局設零分，在主積分上等同敗場。這只證明參考平台提供不同計分模型，不表示本站已採用逐局或全勝加分，亦不決定本站的勝／和／負／輪空數值。本站主積分提案與對手戰績排序必須分開。

現行實作／驗證以[關卡](../competition-review-gates.md)與正式測試為準。

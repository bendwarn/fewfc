---
status: accepted
---

# Use typed Pending Choice answers

待選擇事項（Pending Choice）命令使用封閉且具型別的答案契約，分別表示牌、玩家、陣法與拒絕；每種待選擇事項驗證自己接受的答案變體。此契約取代僅能回答牌的 payload，而非為每個陣法新增命令，或以字串與空牌陣列編碼非牌選擇，因此正規命令、重播、Web DTO 及未來由效果產生的選擇共用明確協定。

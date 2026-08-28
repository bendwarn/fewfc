<template>
  <div class="privacy-page">
    <header class="privacy-header">
      <NuxtLink class="privacy-brand" to="/" aria-label="返回五行戰鬥牌首頁">
        <img src="/header-banner.svg" alt="五行戰鬥牌" width="760" height="120">
      </NuxtLink>
      <div class="privacy-header-actions">
        <NuxtLink to="/">返回首頁</NuxtLink>
        <ThemeSelector />
      </div>
    </header>

    <main class="privacy-main">
      <article class="privacy-policy">
        <p class="privacy-kicker">PRIVACY POLICY</p>
        <h1>隱私權政策</h1>
        <p class="privacy-updated">生效日期：2026 年 8 月 28 日</p>
        <p class="privacy-lead">五行戰鬥牌重視你的隱私。本政策說明線上遊戲服務如何蒐集、使用、保存及保護個人資料，以及你可以如何提出資料請求。</p>

        <section>
          <h2>一、適用範圍</h2>
          <p>本政策適用於由 CFECards 提供的五行戰鬥牌線上服務，包括 <a :href="SITE_URL">{{ SITE_URL }}</a> 的帳號登入、牌組、線上房間及重播功能。連往其他網站後，則適用該網站自己的隱私政策。</p>
        </section>

        <section>
          <h2>二、我們蒐集的資料</h2>
          <ul>
            <li><strong>帳號資料：</strong>Email、玩家名稱、頭像，以及 Google 或 GitHub 提供的帳號識別碼。若使用 Email 與密碼登入，我們只保存密碼雜湊，不保存可讀取的原始密碼。</li>
            <li><strong>遊戲資料：</strong>自訂牌組、房間成員關係、遊戲操作、對戰結果及你選擇保存的重播。</li>
            <li><strong>技術與安全資料：</strong>登入 Session、時間戳記、IP 位址、User-Agent，以及維持服務與防止濫用所需的錯誤或安全紀錄。</li>
            <li><strong>訪客資料：</strong>訪客玩家名稱、牌組與重播。訪客升級正式帳號時，符合條件的資料會轉移至該正式帳號。</li>
          </ul>
        </section>

        <section>
          <h2>三、資料用途</h2>
          <ul>
            <li>建立並驗證玩家身份，讓你可跨裝置使用同一帳號。</li>
            <li>提供牌組、房間、線上對戰、通知及重播功能。</li>
            <li>保護帳號與服務安全、偵測錯誤、濫用或未授權操作。</li>
            <li>維護、除錯及改善服務的可靠性。</li>
          </ul>
          <p>我們不出售個人資料，也不將 Google 或 GitHub 提供的資料用於廣告投放。</p>
        </section>

        <section>
          <h2>四、Google 與 GitHub 登入</h2>
          <p>選擇社群登入時，我們會取得提供登入所需的基本個人資料，例如帳號識別碼、Email、名稱及頭像。Google 登入只要求 OpenID、基本個人資料與 Email；GitHub 登入只要求辨識帳號及取得 Email 所需的權限，不要求儲存庫內容。</p>
          <p>社群登入憑證由 Better Auth 處理，OAuth token 會加密保存。Google 與 GitHub 仍會依各自政策處理你在其服務上的資料：</p>
          <ul>
            <li><a href="https://policies.google.com/privacy" target="_blank" rel="noopener noreferrer">Google 隱私權政策</a></li>
            <li><a href="https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement" target="_blank" rel="noopener noreferrer">GitHub 一般隱私聲明</a></li>
          </ul>
        </section>

        <section>
          <h2>五、服務供應商與資料安全</h2>
          <p>本服務使用 Cloudflare Workers、D1 與 Durable Objects 執行應用程式及保存服務資料。資料可能由 Cloudflare 的全球基礎設施處理。我們採取合理的存取控制、加密與權限限制，但任何網路服務均無法保證絕對安全。</p>
        </section>

        <section>
          <h2>六、Cookie 與本機儲存</h2>
          <p>我們使用必要的登入 Cookie 維持 Session，並可能在瀏覽器保存介面偏好及即時通知狀態。這些資料用於提供你要求的功能；目前不使用第三方廣告追蹤 Cookie。</p>
        </section>

        <section>
          <h2>七、保存、刪除與你的選擇</h2>
          <p>資料會在提供帳號與遊戲功能、安全維護及服務營運所需期間保存。你可要求查詢、更正或刪除帳號與相關個人資料。我們會先確認請求者身份，再依適用法律及維持對戰紀錄完整性所需的範圍處理；部分安全、稽核或去識別化紀錄可能需要保留。</p>
          <p>你也可以在帳號設定中解除 Google 或 GitHub 登入方式，但必須至少保留一種可用的登入方式。</p>
        </section>

        <section>
          <h2>八、政策更新與聯絡方式</h2>
          <p>本政策若有重大變更，會更新本頁內容與生效日期。如對隱私、資料使用或刪除申請有疑問，請寄信至 <a href="mailto:rexkimta@gmail.com">rexkimta@gmail.com</a>。</p>
        </section>
      </article>
    </main>

    <footer class="privacy-footer">
      <span>© 2026 CFECards</span>
      <NuxtLink to="/">五行戰鬥牌首頁</NuxtLink>
    </footer>
  </div>
</template>

<script setup lang="ts">
import { SITE_URL } from '~/lib/site-metadata'

definePageMeta({ layout: 'landing' })
</script>

<style scoped>
.privacy-page { min-height: 100vh; background: var(--app-canvas); }
.privacy-header { display: flex; width: min(calc(100% - 40px), 980px); min-height: 92px; margin: 0 auto; align-items: center; justify-content: space-between; gap: 24px; border-bottom: 1px solid var(--app-border); }
.privacy-brand { display: block; min-width: 0; }.privacy-brand img { display: block; width: min(300px, 46vw); height: auto; border-radius: 7px; box-shadow: var(--app-shadow-sm); }
.privacy-header-actions { display: flex; align-items: center; gap: 18px; }.privacy-header-actions > a { color: var(--app-text-muted); font-size: 13px; text-decoration: none; }.privacy-header-actions > a:hover, .privacy-header-actions > a:focus-visible { color: var(--app-accent-strong); }
.privacy-main { width: min(calc(100% - 40px), 820px); margin: 0 auto; padding: clamp(56px, 8vw, 96px) 0; }
.privacy-policy { border: 1px solid var(--app-border); border-radius: 20px; background: var(--app-surface); padding: clamp(28px, 6vw, 64px); box-shadow: var(--app-shadow-md); }
.privacy-kicker { color: var(--app-accent-strong); font-size: 11px; font-weight: 800; letter-spacing: .16em !important; }.privacy-policy h1 { margin-top: 14px; font-family: var(--font-serif); font-size: clamp(36px, 6vw, 58px); }.privacy-updated { margin-top: 12px; color: var(--app-text-soft); font-size: 12px; }.privacy-lead { margin-top: 28px; color: var(--app-text-muted); font-size: 16px; line-height: 1.9; }
.privacy-policy section { margin-top: 42px; padding-top: 34px; border-top: 1px solid var(--app-border); }.privacy-policy h2 { font-family: var(--font-serif); font-size: 22px; }.privacy-policy p, .privacy-policy li { color: var(--app-text-muted); font-size: 14px; line-height: 1.9; }.privacy-policy section p, .privacy-policy ul { margin-top: 14px; }.privacy-policy ul { display: grid; gap: 8px; padding-left: 22px; }.privacy-policy strong { color: var(--app-text); }.privacy-policy a { color: var(--app-accent-strong); text-underline-offset: 3px; }
.privacy-footer { display: flex; width: min(calc(100% - 40px), 980px); min-height: 100px; margin: 0 auto; align-items: center; justify-content: space-between; border-top: 1px solid var(--app-border); color: var(--app-text-muted); font-size: 11px; }.privacy-footer a { color: inherit; text-decoration: none; }.privacy-footer a:hover, .privacy-footer a:focus-visible { color: var(--app-accent-strong); }
@media (max-width: 620px) { .privacy-header { min-height: 76px; }.privacy-header-actions > a { display: none; }.privacy-main { width: min(calc(100% - 28px), 820px); padding: 36px 0 56px; }.privacy-policy { border-radius: 14px; padding: 26px 20px; }.privacy-policy section { margin-top: 34px; padding-top: 28px; }.privacy-footer { width: min(calc(100% - 28px), 980px); min-height: 90px; } }
</style>

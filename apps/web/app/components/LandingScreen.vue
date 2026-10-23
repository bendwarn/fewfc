<template>
  <div class="landing-page">
    <a class="skip-link" href="#landing-content">跳到主要內容</a>

    <header class="landing-header ui-glass">
      <NuxtLink class="landing-brand" to="/" aria-label="五行戰鬥牌首頁">
        <img :src="'/header-banner.svg'" alt="五行戰鬥牌" width="760" height="120">
      </NuxtLink>
      <nav class="landing-nav" aria-label="首頁導覽">
        <a class="section-link" href="#how-it-works">玩法</a>
        <a class="section-link" href="#features">特色</a>
        <a
          class="official-link"
          :href="OFFICIAL_SITE_URL"
          target="_blank"
          rel="noopener noreferrer"
        >
          五行戰鬥牌官方網站 <span aria-hidden="true">↗</span>
        </a>
        <ThemeSelector />
      </nav>
    </header>

    <main id="landing-content">
      <section class="hero-section" aria-labelledby="hero-title">
        <div class="landing-hero-copy">
          <p class="eyebrow"><span class="element-dots" aria-hidden="true"><i /><i /><i /><i /><i /></span> 線上多人卡牌對戰</p>
          <h1 id="hero-title">以牌為陣，<br><em>決勝五行。</em></h1>
          <p class="landing-hero-description">{{ SITE_DESCRIPTION }}</p>
          <div class="landing-hero-actions">
            <NuxtLink class="landing-primary primary-button" to="/login">立即遊玩 <span aria-hidden="true">→</span></NuxtLink>
            <a class="landing-secondary secondary-button" href="#how-it-works">了解玩法</a>
          </div>
          <dl class="hero-facts ui-glass" aria-label="遊戲摘要">
            <div><dt>2–4</dt><dd>玩家</dd></div>
            <div><dt>即時</dt><dd>線上對戰</dd></div>
            <div><dt>五行</dt><dd>陣法策略</dd></div>
          </dl>
        </div>

        <div class="hero-visual" aria-label="金、木、水、火、土五張卡牌">
          <div class="hero-aura" aria-hidden="true"><span>五行</span></div>
          <figure v-for="card in showcaseCards" :key="`hero-${card.element}`" :class="['hero-card', `hero-${card.element}`]">
            <img :src="card.src" :alt="`${card.label}卡牌`" width="627" height="949">
          </figure>
        </div>
      </section>

      <section id="how-it-works" class="landing-section how-section" aria-labelledby="how-title">
        <div class="section-heading">
          <p class="section-kicker">入局三步</p>
          <h2 id="how-title">三步開局，每手都是選擇</h2>
          <p>從五行卡牌中建立策略，找出屬於你的制勝陣法。</p>
        </div>
        <ol class="steps-grid">
          <li v-for="(step, index) in steps" :key="step.title" class="ui-glass">
            <span class="step-index">0{{ index + 1 }}</span>
            <span class="step-glyph" aria-hidden="true">{{ step.glyph }}</span>
            <h3>{{ step.title }}</h3>
            <p>{{ step.description }}</p>
          </li>
        </ol>
      </section>

      <section id="features" class="landing-section features-section" aria-labelledby="features-title">
        <div class="section-heading compact">
          <p class="section-kicker">步步成局</p>
          <h2 id="features-title">一組牌，展開層層戰術</h2>
        </div>
        <div class="features-grid">
          <article v-for="feature in features" :key="feature.title" class="ui-glass">
            <span class="feature-glyph" aria-hidden="true">{{ feature.glyph }}</span>
            <div>
              <h3>{{ feature.title }}</h3>
              <p>{{ feature.description }}</p>
            </div>
          </article>
        </div>
      </section>

      <section class="landing-section cards-section" aria-labelledby="cards-title">
        <div class="cards-copy">
          <p class="section-kicker">五行萬象</p>
          <h2 id="cards-title">掌握五行，觀局成陣</h2>
          <p>金、木、水、火、土各有節奏。觀察手牌、開啟陣法，在生剋轉化中找到反擊的一手。</p>
          <NuxtLink class="text-link" to="/login">進入線上對戰 <span aria-hidden="true">→</span></NuxtLink>
        </div>
        <div class="card-showcase" role="list" aria-label="五行卡牌展示">
          <figure v-for="card in showcaseCards" :key="card.element" :class="`showcase-${card.element}`" role="listitem">
            <img :src="card.src" :alt="`${card.label}卡牌`" width="627" height="949" loading="lazy">
            <figcaption><span>{{ card.glyph }}</span>{{ card.label }}</figcaption>
          </figure>
        </div>
      </section>
    </main>

    <footer class="landing-footer">
      <span>© 2026 CFECards</span>
      <NuxtLink to="/privacy">隱私權政策</NuxtLink>
      <a :href="OFFICIAL_SITE_URL" target="_blank" rel="noopener noreferrer">五行戰鬥牌官方網站 <span aria-hidden="true">↗</span></a>
    </footer>
  </div>
</template>

<script setup lang="ts">
import ThemeSelector from '~/components/ThemeSelector.vue'
import { OFFICIAL_SITE_URL, SITE_DESCRIPTION } from '~/lib/site-metadata'


const steps = [
  { glyph: '牌', title: '建立你的牌組', description: '從五行與不同等級的卡牌中取捨，準備屬於你的戰術組合。' },
  { glyph: '陣', title: '組合五行陣法', description: '每次出牌都可能連成陣法；判斷時機，開啟攻勢、防守或控場效果。' },
  { glyph: '戰', title: '與玩家線上交鋒', description: '建立或加入房間，在雙人與團隊對戰中觀察、應變，掌握戰局。' },
]

const features = [
  { glyph: '生', title: '五行生剋', description: '卡牌屬性不只是分類，更是預測對手、鋪陳下一手的策略線索。' },
  { glyph: '陣', title: '陣法組合', description: '用同一手牌回應不同局面，在攻守與時機間做出取捨。' },
  { glyph: '眾', title: '多人對戰', description: '支援 1 對 1 雙人對戰與 2 對 2 團隊對戰，與好友一起成局。' },
  { glyph: '錄', title: '戰局重播', description: '儲存已完成的對戰，逐步回顧戰局轉折，看見每個決定的影響。' },
]

const showcaseCards = [
  { element: 'metal', glyph: '金', label: '金行', src: '/cards/metal-1.webp' },
  { element: 'wood', glyph: '木', label: '木行', src: '/cards/wood-2.webp' },
  { element: 'water', glyph: '水', label: '水行', src: '/cards/water-3.webp' },
  { element: 'fire', glyph: '火', label: '火行', src: '/cards/fire-4.webp' },
  { element: 'earth', glyph: '土', label: '土行', src: '/cards/earth-5.webp' },
] as const
</script>

<style scoped>
.landing-page {
  --landing-max: 1240px;
  position: relative;
  isolation: isolate;
  display: flow-root;
  min-height: 100vh;
  overflow: clip;
  background: var(--app-canvas);
}

/* 首屏以山水襯托卡牌，下方逐漸過渡至主題底色，維持長文可讀性。 */
.landing-page::before {
  content: '';
  position: absolute;
  z-index: -1;
  inset: 0 0 auto;
  height: 1080px;
  background: linear-gradient(180deg, rgba(8,24,20,.24), rgba(8,24,20,.06) 60%, var(--app-canvas) 100%), linear-gradient(90deg, rgba(6,20,16,.5), rgba(6,20,16,.18) 47%, rgba(6,20,16,.05)), url('/backgrounds/gate.webp') center top / cover;
}

.skip-link {
  position: fixed;
  top: 8px;
  left: 8px;
  z-index: 100;
  transform: translateY(-150%);
  border-radius: 8px;
  background: var(--app-accent);
  color: var(--app-on-accent);
  padding: 10px 14px;
}

.skip-link:focus { transform: translateY(0); }

.landing-header {
  position: relative;
  z-index: 40;
  display: flex;
  width: min(calc(100% - 40px), var(--landing-max));
  min-height: 76px;
  margin: 18px auto 0;
  padding: 10px 24px;
  border-radius: 22px;
  align-items: center;
  justify-content: space-between;
  gap: 32px;
}

.landing-brand { display: block; min-width: 0; }
.landing-brand img { display: block; width: min(290px, 34vw); height: auto; border-radius: 7px; box-shadow: var(--app-shadow-sm); }
.landing-nav { display: flex; align-items: center; gap: clamp(14px, 2.2vw, 28px); font-size: 13px; }
.landing-nav a { color: var(--app-text); text-decoration: none; }
.landing-nav a:hover, .landing-nav a:focus-visible { color: var(--app-accent-strong); }
.official-link { display: inline-flex; align-items: center; gap: 5px; white-space: nowrap; }
.official-link span { color: var(--app-accent); }

.hero-section {
  position: relative;
  z-index: 1;
  display: grid;
  width: min(calc(100% - 40px), var(--landing-max));
  min-height: 690px;
  margin: 0 auto;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  align-items: center;
  gap: clamp(24px, 4vw, 64px);
  padding: 64px 0 96px;
}

.landing-hero-copy { position: relative; z-index: 3; animation: landing-rise .75s ease-out both; }
.eyebrow, .section-kicker { color: var(--app-accent-strong); font-size: 11px; font-weight: 800; letter-spacing: .2em !important; }
.eyebrow { color: #f1d99e; display: flex; align-items: center; gap: 12px; margin-bottom: 24px; }
.element-dots { display: inline-flex; gap: 4px; }
.element-dots i { width: 6px; height: 6px; border-radius: 50%; background: #a7a9a4; }
.element-dots i:nth-child(2) { background: #5d9b67; }
.element-dots i:nth-child(3) { background: #4c87bd; }
.element-dots i:nth-child(4) { background: #bd5146; }
.element-dots i:nth-child(5) { background: #b8944e; }
.landing-hero-copy h1 { max-width: 620px; color: #f5f0df; text-shadow: 0 3px 22px rgba(0,0,0,.35); font-family: var(--font-serif); font-size: clamp(52px, 6.2vw, 84px); line-height: 1.22; letter-spacing: -.045em !important; }
.landing-hero-copy h1 em { color: #eed695; font-style: normal; }
.landing-hero-description { max-width: 500px; margin-top: 24px; color: #e0e8df; text-shadow: 0 1px 10px #10251d; font-size: clamp(15px, 1.5vw, 18px); line-height: 1.9; }
.landing-hero-actions { display: flex; flex-wrap: wrap; gap: 12px; margin-top: 38px; }
.landing-primary, .landing-secondary { display: inline-flex; min-height: 54px; align-items: center; justify-content: center; gap: 30px; padding: 0 26px; font-size: 14px; font-weight: 700; text-decoration: none; }
.landing-secondary { background: rgba(24,47,38,.55); color: #f4efdf; border-color: rgba(230,246,232,.5); -webkit-backdrop-filter: blur(18px); backdrop-filter: blur(18px); }
.landing-secondary:hover { color: #f3d899; border-color: #f3d899; }
.hero-facts { display: inline-flex; gap: 0; margin: 38px 0 0; padding: 16px 0; border-radius: 20px; }
.hero-facts div { min-width: 104px; padding: 0 24px; }
.hero-facts div + div { border-left: 1px solid var(--ui-glass-stroke); }
.hero-facts dt { color: var(--app-text); font-family: var(--font-serif); font-size: 18px; font-weight: 900; }
.hero-facts dd { margin: 6px 0 0; color: var(--app-text); opacity: .8; font-size: 11px; }

.hero-visual { position: relative; min-height: 490px; animation: landing-fade 1s .15s ease-out both; }
.hero-aura {
  position: absolute;
  top: 50%;
  left: 50%;
  display: grid;
  width: min(29vw, 350px);
  aspect-ratio: 1;
  transform: translate(-50%, -50%);
  place-items: center;
  border: 1px solid color-mix(in srgb, var(--app-accent) 35%, transparent);
  border-radius: 50%;
  background: radial-gradient(circle, color-mix(in srgb, var(--app-accent-soft) 84%, transparent), transparent 68%);
  box-shadow: 0 0 90px color-mix(in srgb, var(--app-accent) 16%, transparent);
}
.hero-aura::before, .hero-aura::after { position: absolute; border: 1px solid color-mix(in srgb, var(--app-accent) 20%, transparent); border-radius: inherit; content: ''; }
.hero-aura::before { inset: 11%; }
.hero-aura::after { inset: -13%; box-shadow: 0 0 40px rgba(238,219,151,.1); }
.hero-aura span { color: color-mix(in srgb, var(--app-accent-strong) 65%, transparent); font-family: var(--font-serif); font-size: clamp(34px, 5vw, 68px); font-weight: 900; }
.hero-card { position: absolute; top: 50%; left: 50%; width: clamp(108px, 11vw, 154px); margin: 0; filter: drop-shadow(0 22px 25px rgba(0, 0, 0, .3)); }
.hero-card img { display: block; width: 100%; height: auto; border-radius: 5.2% / 3.4%; animation: card-drift 5s ease-in-out infinite; }
.hero-metal { z-index: 1; transform: translate(-184%, -47%) rotate(-22deg); }
.hero-wood { z-index: 2; transform: translate(-113%, -59%) rotate(-11deg); }
.hero-water { z-index: 5; transform: translate(-50%, -64%) rotate(-1deg); }
.hero-fire { z-index: 4; transform: translate(14%, -57%) rotate(11deg); }
.hero-earth { z-index: 3; transform: translate(82%, -45%) rotate(22deg); }
.hero-wood img { animation-delay: -.8s; }
.hero-water img { animation-delay: -1.6s; }
.hero-fire img { animation-delay: -2.4s; }
.hero-earth img { animation-delay: -3.2s; }

.landing-section { position: relative; z-index: 1; width: min(calc(100% - 40px), var(--landing-max)); margin: 0 auto; padding: 88px 0; }
.section-heading { max-width: 670px; margin: 0 auto 56px; text-align: center; }
.section-heading.compact { margin-bottom: 46px; }
.section-heading h2, .cards-copy h2 { margin-top: 14px; font-family: var(--font-serif); font-size: clamp(32px, 4.5vw, 52px); line-height: 1.28; }
.section-heading > p:last-child, .cards-copy > p { margin-top: 18px; color: var(--app-text-muted); font-size: 14px; line-height: 1.8; }
.how-section { padding-top: 60px; }
.steps-grid { display: grid; margin: 0; grid-template-columns: repeat(3, 1fr); gap: 18px; padding: 0; list-style: none; }
.steps-grid li { position: relative; min-height: 300px; overflow: hidden; border-radius: 24px; padding: 30px; }
.steps-grid li::after { position: absolute; right: -50px; bottom: -70px; width: 180px; height: 180px; border: 1px solid color-mix(in srgb, var(--app-accent) 20%, transparent); border-radius: 50%; content: ''; }
.step-index { color: var(--app-accent-strong); font-size: 11px; font-weight: 800; }
.step-glyph, .feature-glyph { display: grid; place-items: center; border: 1px solid color-mix(in srgb, var(--app-accent) 45%, var(--app-border)); color: var(--app-accent-strong); font-family: var(--font-serif); font-weight: 900; }
.step-glyph { width: 62px; height: 62px; margin-top: 28px; border-radius: 50%; font-size: 25px; }
.steps-grid h3 { margin-top: 26px; font-family: var(--font-serif); font-size: 20px; }
.steps-grid p, .features-grid p { margin-top: 12px; color: color-mix(in srgb, var(--app-text) 84%, var(--app-surface)); font-size: 13px; line-height: 1.75; }

.features-section { border-top: 1px solid var(--app-border); }
.features-grid { display: grid; grid-template-columns: repeat(2, 1fr); gap: 18px; }
.features-grid article { display: grid; min-height: 190px; grid-template-columns: 64px 1fr; align-items: start; gap: 24px; border-radius: 24px; padding: clamp(28px, 4vw, 44px); }
.feature-glyph { width: 58px; height: 58px; border-radius: 50%; font-size: 22px; }
.features-grid h3 { font-family: var(--font-serif); font-size: 21px; }

.cards-section { display: grid; grid-template-columns: minmax(260px, .72fr) minmax(0, 1.28fr); align-items: center; gap: clamp(48px, 7vw, 100px); }
.cards-copy { max-width: 410px; }
.text-link { display: inline-flex; gap: 20px; margin-top: 28px; color: var(--app-accent-strong); font-size: 13px; font-weight: 800; text-decoration: none; }
.text-link:hover, .text-link:focus-visible { text-decoration: underline; text-underline-offset: 5px; }
.card-showcase { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); align-items: end; gap: clamp(8px, 1.3vw, 16px); }
.card-showcase figure { margin: 0; transition: transform .24s ease; }
.card-showcase figure:nth-child(even) { transform: translateY(24px); }
.card-showcase figure:hover { transform: translateY(-10px); }
.card-showcase figure:nth-child(even):hover { transform: translateY(14px); }
.card-showcase img { display: block; width: 100%; height: auto; border-radius: 5.2% / 3.4%; box-shadow: 0 18px 34px rgba(0, 0, 0, .28); }
.card-showcase figcaption { display: flex; align-items: center; justify-content: center; gap: 6px; margin-top: 14px; color: var(--app-text-muted); font-size: 10px; font-weight: 700; }
.card-showcase figcaption span { display: grid; width: 20px; height: 20px; place-items: center; border-radius: 50%; color: #fff; font-family: var(--font-serif); font-size: 10px; }
.showcase-metal figcaption span { background: #777a76; }
.showcase-wood figcaption span { background: #568a5f; }
.showcase-water figcaption span { background: #3f79aa; }
.showcase-fire figcaption span { background: #a8463d; }
.showcase-earth figcaption span { background: #9b7637; }

.landing-footer { position: relative; z-index: 1; display: flex; width: min(calc(100% - 40px), var(--landing-max)); min-height: 100px; margin: 0 auto; align-items: center; justify-content: space-between; gap: 20px; border-top: 1px solid var(--app-border); color: var(--app-text-muted); font-size: 11px; }
.landing-footer a { color: var(--app-text-muted); text-decoration: none; }
.landing-footer a:hover, .landing-footer a:focus-visible { color: var(--app-accent-strong); }

@keyframes landing-rise { from { opacity: 0; transform: translateY(18px); } }
@keyframes landing-fade { from { opacity: 0; transform: scale(.98); } }
@keyframes card-drift { 0%, 100% { transform: translateY(0); } 50% { transform: translateY(-9px); } }

@media (max-width: 920px) {
  .section-link { display: none; }
  .hero-section { min-height: auto; grid-template-columns: 1fr; padding-top: 64px; }
  .landing-hero-copy { max-width: 680px; }
  .hero-visual { min-height: 400px; }
  .hero-aura { width: min(46vw, 340px); }
  .hero-card { width: clamp(110px, 18vw, 150px); }
  .cards-section { grid-template-columns: 1fr; }
  .cards-copy { max-width: 670px; }
}

@media (max-width: 680px) {
  .landing-header { width: min(calc(100% - 28px), var(--landing-max)); min-height: 68px; gap: 10px; padding: 10px 12px; border-radius: 18px; }
  .landing-brand img { width: min(180px, 46vw); }
  .landing-nav { gap: 10px; }
  .official-link { max-width: 124px; white-space: normal; font-size: 10px; line-height: 1.35; }
  .hero-section, .landing-section, .landing-footer { width: min(calc(100% - 32px), var(--landing-max)); }
  .hero-section { gap: 22px; padding: 40px 0 36px; }
  .landing-hero-copy h1 { font-size: clamp(44px, 13vw, 60px); }
  .landing-hero-description { font-size: 15px; }
  .landing-hero-actions > * { flex: 1 1 150px; }
  .hero-facts { display: flex; margin-top: 28px; }
  .hero-facts div { min-width: 0; flex: 1; padding: 0 18px; }
  .hero-visual { min-height: 310px; }
  .hero-aura { width: min(58vw, 250px); }
  .hero-card { width: clamp(64px, 20.5vw, 110px); }
  .landing-section { padding: 82px 0; }
  .steps-grid, .features-grid { grid-template-columns: 1fr; }
  .steps-grid li { min-height: 270px; }
  .features-grid article { min-height: 165px; grid-template-columns: 52px 1fr; gap: 18px; padding: 26px 22px; }
  .feature-glyph { width: 48px; height: 48px; }
  .features-section { padding-inline: 0; }
  .card-showcase { grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 7px; padding: 18px 0 38px; }
  .card-showcase figcaption { flex-direction: column; }
  .landing-footer { min-height: 120px; flex-direction: column; align-items: flex-start; justify-content: center; }
}
</style>

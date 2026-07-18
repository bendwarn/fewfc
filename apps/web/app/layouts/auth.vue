<template>
  <div class="auth-shell">
    <section class="login-hero">
      <div class="hero-copy">
        <p class="kicker"><span /> 五行交鋒，陣法成局</p>
        <h1>以牌為陣，<br><em>決勝五行。</em></h1>
        <p class="hero-description">運用金、木、水、火、土的生剋關係，組合陣法、洞察對手，在每一次出牌中掌握戰局。</p>
        <div class="element-orbit" aria-hidden="true">
          <span class="element metal">金</span><span class="element wood">木</span><span class="element water">水</span><span class="element fire">火</span><span class="element earth">土</span><div class="orbit-core">五行</div>
        </div>
      </div>
      <footer class="hero-footer"><span>© 2026 CFECards</span><span>遊戲規則 · 隱私權</span></footer>
    </section>
    <section class="login-panel"><slot /></section>
  </div>
</template>

<style>
@reference "../assets/css/main.css";

.auth-shell { @apply grid min-h-screen grid-cols-[minmax(0,1.2fr)_minmax(420px,.8fr)] bg-ink; }
.login-hero { @apply relative grid min-h-screen content-center overflow-hidden bg-[#122019] px-[clamp(36px,9vw,150px)] py-16; }
.login-hero::before { content: ''; @apply absolute inset-0 opacity-70; background: radial-gradient(circle at 35% 35%, rgba(103,155,98,.22), transparent 38%), radial-gradient(circle at 70% 68%, rgba(191,148,64,.18), transparent 35%); }
.hero-copy { @apply relative z-1 max-w-xl; }
.kicker { @apply mb-5 flex items-center gap-3 text-xs font-bold tracking-[.2em] text-gold-light; }
.kicker span { @apply h-px w-9 bg-gold; }
.hero-copy h1 { @apply font-serif text-5xl leading-tight text-[#f1ead9] max-[1100px]:text-4xl; }
.hero-copy em { @apply not-italic text-gold-light; }
.hero-description { @apply mt-6 max-w-md text-sm leading-7 text-[#aeb9af]; }
.element-orbit { @apply relative mt-12 h-52 w-52 rounded-full border border-[#526351]; }
.element { @apply absolute grid size-9 place-items-center rounded-full border border-[#b99550] bg-[#18201b] text-xs text-gold-light; }
.metal { top: -18px; left: calc(50% - 18px); }.wood { top: 28px; right: -18px; }.water { right: 8px; bottom: 8px; }.fire { bottom: 8px; left: 8px; }.earth { top: 28px; left: -18px; }
.orbit-core { @apply absolute inset-[58px] grid place-items-center rounded-full border border-[#b99550] bg-[#1d2a21] font-serif text-gold-light; }
.hero-footer { @apply absolute right-12 bottom-8 left-12 flex justify-between text-[10px] text-[#7f8982]; }
.login-panel { @apply grid min-h-screen place-items-center bg-[#e9e2d1] px-6 py-12; }
.auth-card { @apply w-full max-w-md text-[#18201c]; }
.mobile-brand { @apply mb-9 flex items-center gap-3 font-serif text-lg; }
.brand-mark { @apply grid size-[38px] rotate-45 place-items-center border border-[#a57d35] font-serif font-black text-[#8a622b]; }.brand-mark::first-letter { transform: rotate(-45deg); }
.auth-card h2 { @apply font-serif text-3xl; }.auth-card .muted { @apply mt-3 text-sm leading-6 text-[#626259]; }
.auth-card form { @apply mt-8 grid gap-2; }.auth-card label { @apply mt-3 text-xs font-bold text-[#4d4a43]; }
.input-wrap { @apply flex items-center gap-2 border border-[#bdb5a4] bg-[#f9f5eb] px-3 focus-within:border-[#a57d35] focus-within:shadow-[0_0_0_2px_rgba(165,125,53,.12)]; }.input-wrap span { @apply text-xs text-[#8a7d68]; }.input-wrap input { @apply min-w-0 flex-1 border-0 bg-transparent py-3 outline-0; }
.auth-shell .primary-button { @apply inline-flex min-h-10 items-center justify-center gap-2 border border-[#a57d35] bg-[#b99550] px-4 py-2 text-sm font-bold text-[#151812] hover:bg-[#cba85e]; }.auth-shell .ghost-button, .auth-shell .secondary-button { @apply inline-flex min-h-10 items-center justify-center gap-2 border border-[#8c8474] bg-transparent px-4 py-2 text-sm text-[#282b28] hover:border-[#a57d35]; }
.login-button { @apply mt-5 w-full justify-between; }.auth-mode-button { @apply mt-5 border-0 bg-transparent text-sm text-[#695b45] underline underline-offset-4; }.divider { @apply my-7 flex items-center gap-3 text-xs text-[#817869]; }.divider::before, .divider::after { content: ''; @apply h-px flex-1 bg-[#c5bca9]; }.auth-card .ghost-button { @apply w-full; }.terms { @apply mt-5 text-center text-[10px] text-[#82796b]; }.form-error { @apply mt-2 text-sm text-[#ae3e34]; }

@media (max-width: 900px) { .auth-shell { grid-template-columns: 1fr; }.login-hero { display: none; }.login-panel { min-height: 100vh; }.mobile-brand { display: none; } }
</style>

<template>
  <div class="auth-shell page-scene" data-scene="inn">
    <PublicHeader />
    <main class="auth-content">
      <section class="login-hero" aria-label="五行戰鬥牌">
        <div class="hero-copy">
          <p class="kicker">五行之間 · 以牌會友</p>
          <h1>入席，<br><em>下一局等你。</em></h1>
          <p class="hero-description">邀好友相聚，或結識新的對手。<br>從這裡開始你的五行對局。</p>
          <div class="auth-elements" aria-hidden="true"><span>金</span><span>木</span><span>水</span><span>火</span><span>土</span></div>
        </div>
      </section>
      <section class="login-panel"><slot /></section>
    </main>
    <footer class="hero-footer"><span>© 2026 CFECards</span><NuxtLink to="/privacy">隱私權政策</NuxtLink></footer>
  </div>
</template>

<script setup lang="ts">
import PublicHeader from '~/components/PublicHeader.vue'
</script>

<style>
.auth-shell { min-height: 100dvh; display: flex; flex-direction: column; }
.auth-shell::before { background: linear-gradient(90deg, rgba(10,22,17,.74), rgba(10,22,17,.2)), var(--page-scene-image) center / cover; }
.auth-shell .public-header { background: rgba(23,40,32,.72); color: #f4f0e2; }
.auth-shell .public-header nav > a { color: #f4f0e2; }
.auth-content { display: grid; grid-template-columns: minmax(0,1.1fr) minmax(360px,.9fr); gap: 64px; width: min(1180px,100%); margin: auto; padding: 48px 32px; }
.login-hero { display: grid; align-content: center; min-width: 0; }
.hero-copy { max-width: 560px; text-shadow: 0 2px 24px rgba(0,0,0,.45); }
.kicker { margin-bottom: 24px; font-size: 12px; color: #eed699; }
.hero-copy h1 { font: 900 clamp(40px,5vw,68px) / 1.4 var(--font-serif); color: #f4f0e2; }
.hero-copy em { color: #eed699; font-style: normal; }
.hero-description { margin-top: 24px; font-size: 15px; line-height: 2; color: #dce5dd; }
.auth-elements { display: flex; gap: 16px; margin-top: 36px; }
.auth-elements span { display: grid; place-items: center; width: 40px; height: 40px; border: 1px solid rgba(238,214,153,.38); border-radius: 50%; color: #eed699; font: 16px var(--font-serif); background: rgba(23,40,32,.44); }
.login-panel { min-width: 0; display: flex; align-items: center; justify-content: center; }
.auth-card { width: 100%; max-width: 440px; padding: 32px; border: 1px solid var(--ui-glass-stroke); border-radius: 28px; background: var(--ui-glass-fill); box-shadow: var(--ui-glass-shadow); backdrop-filter: blur(22px); -webkit-backdrop-filter: blur(22px); color: var(--app-text); }
.mobile-brand { display: none; }
.auth-card h2 { font: 700 30px / 1.4 var(--font-serif); }
.auth-card .muted { margin-top: 12px; font-size: 13px; line-height: 1.8; color: var(--app-text-muted); }
.auth-card form { margin-top: 24px; display: grid; gap: 8px; }
.auth-card label { margin-top: 8px; font-size: 12px; font-weight: 600; }
.input-wrap { display: flex; align-items: center; gap: 10px; padding: 0 14px; border: 1px solid var(--ui-glass-stroke); border-radius: 12px; background: color-mix(in srgb,var(--app-input) 90%,transparent); }
.input-wrap:focus-within { outline: 2px solid var(--app-accent); outline-offset: 3px; }
.input-wrap span { font-size: 12px; color: var(--app-text-muted); }
.input-wrap input { min-width: 0; flex: 1; border: 0; background: transparent; padding: 13px 0; outline: 0; color: var(--app-text); }
.login-button { margin-top: 16px; width: 100%; justify-content: space-between; }
.auth-mode-button { margin-top: 18px; padding: 5px 0; border: 0; background: transparent; font-size: 13px; text-decoration: underline; text-underline-offset: 5px; color: var(--app-accent-strong); }
.divider { margin: 22px 0 16px; display: flex; align-items: center; gap: 12px; font-size: 11px; color: var(--app-text-muted); }
.divider::before, .divider::after { content: ''; height: 1px; flex: 1; background: var(--app-border-strong); opacity: .5; }
.auth-card .ghost-button { width: 100%; min-height: 44px; }
.social-button + .social-button { margin-top: 8px; }
.terms { margin-top: 18px; text-align: center; font-size: 10px; line-height: 1.8; color: var(--app-text-muted); }
.form-error { margin-top: 8px; font-size: 13px; line-height: 1.7; color: var(--app-danger); }
.hero-footer { display: flex; justify-content: space-between; gap: 20px; width: min(1180px,100%); margin-inline: auto; padding: 20px 32px; color: #dce5dd; font-size: 11px; }
.hero-footer a { color: inherit; text-decoration: none; }
.hero-footer a:hover { text-decoration: underline; }
@media(max-width:900px) { .auth-content { grid-template-columns: minmax(0,1fr); gap: 0; padding: 32px 16px; } .login-hero { display: none; } .auth-card { padding: 28px 24px; border-radius: 24px; } .hero-footer { padding: 20px 24px; } }
</style>

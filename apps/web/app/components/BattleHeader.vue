<template>
  <div class="battle-header-reveal">
    <header class="site-header"><slot /></header>
  </div>
</template>

<style>
.battle-header-reveal { flex: none; }

/* 對局導覽浮於畫面上；頂端感應帶與鍵盤焦點皆可喚出，不佔戰場高度。 */
@media (min-width: 901px) {
  .app-shell:has(> .game-page[data-in-game="true"]) > .battle-header-reveal {
    position: fixed;
    inset: 0 0 auto;
    z-index: 45;
    height: 16px;
  }
  .app-shell:has(> .game-page[data-in-game="true"]) > .battle-header-reveal > .site-header {
    transform: translateY(calc(-100% - 12px));
    opacity: 0;
    pointer-events: none;
    transition: transform .2s ease, opacity .2s ease;
  }
  .app-shell:has(> .game-page[data-in-game="true"]) > .battle-header-reveal:is(:hover, :focus-within, :has([aria-expanded="true"])) > .site-header {
    transform: translateY(0);
    opacity: 1;
    pointer-events: auto;
  }
}

@media (prefers-reduced-motion: reduce) {
  .battle-header-reveal > .site-header { transition: none !important; }
}
</style>

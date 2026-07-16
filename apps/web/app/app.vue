<template>
  <div class="app-shell" :class="`screen-${screen}`">
    <NuxtRouteAnnouncer />
    <NuxtPage class="route-page" />

    <header class="site-header">
      <button class="brand" type="button" aria-label="回到首頁" @click="goHome">
        <img class="brand-banner" src="/header-banner.svg" alt="五行戰鬥牌">
      </button>

      <nav v-if="routeReady && !isAuthenticationScreen" class="header-actions" aria-label="帳號選單">
        <span class="connection" :class="{ offline: appConnectionText !== '已連線' }">
          <i /> {{ appConnectionText }}
        </span>
        <div ref="profileMenuContainer" class="profile-menu-container">
          <button class="profile-button" type="button" @click="profileOpen = !profileOpen">
            <span class="avatar">{{ playerInitial }}</span>
            <span>{{ displayName }}</span>
            <span aria-hidden="true">⌄</span>
          </button>
          <div v-if="profileOpen" class="profile-menu">
            <button type="button" @click="openDeckEditor">個人牌組</button>
            <button type="button" @click="openReplays">重播紀錄</button>
            <button type="button" @click="logout">登出</button>
          </div>
        </div>
      </nav>
    </header>

    <main v-if="!routeReady" class="route-state">
      <div class="route-state-content">
        <span class="route-spinner" aria-hidden="true" />
        <h1>正在載入</h1>
        <p>正在恢復玩家與房間狀態。</p>
      </div>
    </main>

    <main v-else-if="roomRouteError" class="route-state">
      <div class="route-state-content">
        <h1>{{ roomRouteError }}</h1>
        <button class="primary-button" type="button" @click="returnToLobby">
          返回房間大廳
        </button>
      </div>
    </main>

    <main v-else-if="isAuthenticationScreen" class="login-layout">
      <section class="login-hero">
        <div class="hero-copy">
          <p class="kicker"><span /> 五行交鋒，陣法成局</p>
          <h1>以牌為陣，<br><em>決勝五行。</em></h1>
          <p class="hero-description">
            運用金、木、水、火、土的生剋關係，組合陣法、洞察對手，
            在每一次出牌中掌握戰局。
          </p>
          <div class="element-orbit" aria-hidden="true">
            <span class="element metal">金</span>
            <span class="element wood">木</span>
            <span class="element water">水</span>
            <span class="element fire">火</span>
            <span class="element earth">土</span>
            <div class="orbit-core">五行</div>
          </div>
        </div>
        <footer class="hero-footer">
          <span>© 2026 CFECards</span>
          <span>遊戲規則 · 隱私權</span>
        </footer>
      </section>

      <section class="login-panel">
        <div class="auth-card">
          <template v-if="screen === 'login'">
          <div class="mobile-brand"><span class="brand-mark">五</span> 五行戰牌</div>
          <h2>{{ authMode === 'sign-in' ? '登入對戰' : '建立帳號' }}</h2>
          <p class="muted">
            {{ authMode === 'sign-in' ? '使用 Email 登入，繼續你的對戰紀錄。' : '建立可在不同裝置使用的玩家身份。' }}
          </p>

          <form @submit.prevent="login">
            <template v-if="authMode === 'sign-up'">
              <label for="player-name">玩家名稱</label>
              <div class="input-wrap">
                <span aria-hidden="true">人</span>
                <input
                  id="player-name"
                  v-model.trim="nameInput"
                  type="text"
                  maxlength="16"
                  autocomplete="nickname"
                  placeholder="顯示名稱"
                >
              </div>
            </template>

            <label for="player-email">Email</label>
            <div class="input-wrap">
              <span aria-hidden="true">@</span>
              <input
                id="player-email"
                v-model.trim="emailInput"
                type="email"
                autocomplete="email"
                placeholder="you@example.com"
                autofocus
              >
            </div>

            <label for="player-password">密碼</label>
            <div class="input-wrap">
              <span aria-hidden="true">密</span>
              <input
                id="player-password"
                v-model="passwordInput"
                type="password"
                :autocomplete="authMode === 'sign-in' ? 'current-password' : 'new-password'"
                placeholder="至少 10 個字元"
              >
            </div>
            <p v-if="loginError" class="form-error">{{ loginError }}</p>
            <button class="primary-button login-button" type="submit" :disabled="authBusy">
              {{ authBusy ? '處理中…' : authMode === 'sign-in' ? '登入' : '註冊並登入' }}
              <span aria-hidden="true">→</span>
            </button>
          </form>

          <button class="auth-mode-button" type="button" @click="toggleAuthMode">
            {{ authMode === 'sign-in' ? '還沒有帳號？建立帳號' : '已經有帳號？返回登入' }}
          </button>

          <button
            v-if="localPasswordResetEnabled && authMode === 'sign-in'"
            class="auth-mode-button"
            type="button"
            @click="openLocalPasswordReset"
          >
            重設本機密碼
          </button>

          <div class="divider"><span>或使用訪客身份</span></div>
          <button class="ghost-button" type="button" :disabled="authBusy" @click="guestLogin">
            以訪客身份遊玩
          </button>
          <p class="terms">繼續即表示你同意遊戲規範與使用條款。</p>
          </template>

          <template v-else>
            <div class="mobile-brand"><span class="brand-mark">五</span> 五行戰牌</div>
            <h2>重設本機密碼</h2>
            <p class="muted">
              此入口僅適用於已啟用的本機開發環境，會直接更新既有的 Email 密碼登入憑證。
            </p>

            <form @submit.prevent="resetLocalPassword">
              <label for="reset-password-email">Email</label>
              <div class="input-wrap">
                <span aria-hidden="true">@</span>
                <input
                  id="reset-password-email"
                  v-model.trim="resetPasswordEmail"
                  type="email"
                  autocomplete="email"
                  placeholder="you@example.com"
                  autofocus
                >
              </div>

              <label for="reset-password-new">新密碼</label>
              <div class="input-wrap">
                <span aria-hidden="true">密</span>
                <input
                  id="reset-password-new"
                  v-model="resetPasswordInput"
                  type="password"
                  autocomplete="new-password"
                  minlength="10"
                  maxlength="128"
                  placeholder="至少 10 個字元"
                >
              </div>

              <label for="reset-password-confirm">確認新密碼</label>
              <div class="input-wrap">
                <span aria-hidden="true">密</span>
                <input
                  id="reset-password-confirm"
                  v-model="resetPasswordConfirmation"
                  type="password"
                  autocomplete="new-password"
                  minlength="10"
                  maxlength="128"
                  placeholder="再次輸入新密碼"
                >
              </div>

              <p v-if="resetPasswordError" class="form-error" role="alert">{{ resetPasswordError }}</p>
              <button class="primary-button login-button" type="submit" :disabled="resetPasswordBusy">
                {{ resetPasswordBusy ? '重設中…' : '重設密碼並登入' }}
                <span aria-hidden="true">→</span>
              </button>
            </form>

            <button class="auth-mode-button" type="button" :disabled="resetPasswordBusy" @click="returnToLogin">
              返回登入
            </button>
          </template>
        </div>
      </section>
    </main>

    <main v-else-if="screen === 'deck'" class="deck-page">
      <section class="setup-card deck-editor">
        <div class="card-heading">
          <span class="step-number">牌</span>
          <div>
            <h1>個人牌組</h1>
            <p>每種牌依屬性與等級調整張數；總數 {{ deckRules?.exactCardCount ?? '—' }}、等級總和最多 {{ deckRules?.maximumLevelTotal ?? '—' }}。</p>
          </div>
        </div>

        <label for="deck-name">牌組名稱</label>
        <input id="deck-name" v-model.trim="deckDraft.name" class="text-input" maxlength="24">

        <div class="deck-grid" role="table" aria-label="個人牌組卡牌張數">
          <div />
          <strong v-for="level in DECK_LEVELS" :key="`level-${level}`">{{ level }} 級</strong>
          <template v-for="element in DECK_ELEMENTS" :key="element">
            <strong>{{ deckElementLabel(element) }}</strong>
            <div v-for="level in DECK_LEVELS" :key="`${element}-${level}`" class="deck-count-control">
              <button
                type="button"
                :aria-label="`減少${deckElementLabel(element)}${level}級`"
                :disabled="deckCardCount(element, level) === 0"
                @click="adjustDeckCard(element, level, -1)"
              >−</button>
              <span :aria-label="`${deckElementLabel(element)}${level}級張數`">{{ deckCardCount(element, level) }}</span>
              <button
                type="button"
                :aria-label="`增加${deckElementLabel(element)}${level}級`"
                :disabled="deckCardCount(element, level) >= deckCardLimit(element, level)"
                @click="adjustDeckCard(element, level, 1)"
              >＋</button>
            </div>
          </template>
        </div>

        <div class="deck-validation" :class="{ invalid: !deckValidation.valid }">
          <strong>{{ deckValidation.cardCount }} / {{ deckRules?.exactCardCount ?? '—' }} 張</strong>
          <strong>{{ deckValidation.levelTotal }} / {{ deckRules?.maximumLevelTotal ?? '—' }} 級</strong>
          <span>{{ deckSource === 'custom' ? '目前使用自訂牌組' : '目前使用內建預組' }}</span>
        </div>
        <ul v-if="deckValidation.errors.length" class="deck-validation-errors" aria-live="polite">
          <li v-for="error in deckValidation.errors" :key="error">{{ error }}</li>
        </ul>
        <p v-if="deckError" class="form-error">{{ deckError }}</p>
        <p
          v-if="deckExportStatus"
          class="deck-transfer-status"
          :class="{ 'form-error': deckExportFailed }"
          :role="deckExportFailed ? 'alert' : 'status'"
        >{{ deckExportStatus }}</p>

        <div class="deck-management-actions">
          <button
            ref="deckImportTrigger"
            class="secondary-button"
            type="button"
            :disabled="deckBusy"
            @click="openDeckImport"
          >
            匯入牌組
          </button>
          <button class="secondary-button" type="button" :disabled="deckBusy" @click="exportDeck">
            匯出牌組
          </button>
          <button class="secondary-button" type="button" :disabled="deckBusy" @click="resetDeck">
            重設為預組
          </button>
        </div>
        <div class="setup-actions">
          <button class="secondary-button" type="button" :disabled="deckBusy" @click="returnToLobby">
            返回房間
          </button>
          <button
            class="primary-button"
            type="button"
            :disabled="deckBusy || !deckValidation.valid"
            @click="saveDeck"
          >
            {{ deckBusy ? '儲存中…' : '儲存牌組' }}
          </button>
        </div>

        <div
          v-if="deckImportOpen"
          class="room-settings-layer"
          @click.self="closeDeckImport"
        >
          <form
            class="setup-card deck-import-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="deck-import-title"
            aria-describedby="deck-import-help"
            @keydown.esc.prevent="closeDeckImport"
            @keydown.tab="trapDeckImportFocus"
            @submit.prevent="applyDeckImport"
          >
            <div class="card-heading">
              <span class="step-number">入</span>
              <div>
                <h2 id="deck-import-title">匯入牌組</h2>
                <div id="deck-import-help" class="deck-import-examples">
                  <p>可貼上以下任一格式：</p>
                  <code aria-label="Tab 與換行格式範例">4	1	1	3	1
4	1	1	2	2
4	4	4	3	3
4	1	1	3	3
4	1	1	1	3</code>
                  <code aria-label="連續數字格式範例">4113141122444334113341113</code>
                </div>
              </div>
            </div>

            <label for="deck-import-text">牌組張數</label>
            <textarea
              id="deck-import-text"
              ref="deckImportInput"
              v-model="deckImportText"
              class="deck-import-text"
              rows="5"
              spellcheck="false"
            />
            <p v-if="deckImportError" class="form-error" role="alert">{{ deckImportError }}</p>

            <div class="setup-actions">
              <button class="secondary-button" type="button" @click="closeDeckImport">取消</button>
              <button class="primary-button" type="submit">套用</button>
            </div>
          </form>
        </div>
      </section>
    </main>

    <main v-else-if="screen === 'replays'" class="lobby-page">
      <section class="lobby-content" aria-label="重播紀錄">
        <div class="lobby-heading"><h1>重播紀錄</h1><button class="ghost-button" type="button" @click="returnToLobby">返回房間</button></div>
        <p v-if="replayError" class="form-error">{{ replayError }}</p>
        <p v-if="replayReplaceSource" class="muted">已達 10 局，選擇要替換的紀錄。</p>
        <article v-for="replay in replays" :key="replay.replayId" class="room-card">
          <div><strong>{{ replay.roomName }}</strong><p>{{ replay.players.map(player => player.displayName).join('、') }} · {{ replay.finishedAt }} · {{ replayResultText(replay.result) }}</p></div>
          <div class="result-actions">
            <button class="ghost-button" type="button" @click="openReplay(replay.replayId)">觀看</button>
            <button class="ghost-button" type="button" @click="copyReplayLink(replay.replayId)">複製連結</button>
            <button v-if="replayReplaceSource" class="primary-button" type="button" @click="replaceReplay(replay.replayId)">以本局替換</button>
            <button v-else class="ghost-button" type="button" @click="deleteReplay(replay.replayId)">刪除</button>
          </div>
        </article>
        <p v-if="!replays.length && !replayLoading" class="muted">尚未儲存任何重播。</p>
      </section>
    </main>

    <main v-else-if="screen === 'replay'" class="lobby-page">
      <section class="lobby-content" aria-label="重播播放器">
        <div class="lobby-heading"><h1>重播</h1><button class="ghost-button" type="button" @click="openReplays">返回重播清單</button></div>
        <template v-if="replayFrame">
          <p>第 {{ replayFrame.currentStep }} / {{ replayFrame.totalSteps }} 步</p>
          <div class="result-actions">
            <button class="ghost-button" :disabled="replayFrame.currentStep === 0" @click="loadReplayFrame(0)">第一步</button>
            <button class="ghost-button" :disabled="replayFrame.currentStep === 0" @click="loadReplayFrame(replayFrame.currentStep - 1)">上一步</button>
            <button class="ghost-button" :disabled="replayFrame.currentStep === replayFrame.totalSteps" @click="loadReplayFrame(replayFrame.currentStep + 1)">下一步</button>
            <button class="ghost-button" :disabled="replayFrame.currentStep === replayFrame.totalSteps" @click="loadReplayFrame(replayFrame.totalSteps)">最後一步</button>
            <button class="ghost-button" @click="copyReplayLink(replayRouteId)">分享連結</button>
          </div>
          <div class="battle-layout replay-layout">
            <section
              class="battlefield replay-battlefield"
              :class="{ 'four-player': replaySeats.length === 4 }"
              aria-label="重播戰場（唯讀）"
            >
              <div
                v-for="seat in replaySeats"
                :key="seat.player"
                class="player-seat"
                :class="[`seat-${seat.position}`, { acting: replayFrame.state.currentPlayer === seat.player }]"
              >
                <div class="player-identity">
                  <span class="avatar">{{ replayPlayerLabel(seat.player).slice(0, 1) }}</span>
                  <div>
                    <strong>{{ replayPlayerLabel(seat.player) }}</strong>
                    <small>{{ replayTeamHp(seat.player) }} HP</small>
                    <small v-if="replayPouch(seat.player)">錦囊 · {{ replayPouch(seat.player)?.label }}</small>
                    <small v-if="replayCoveredCards(seat.player).length">蓋牌 · {{ replayCoveredCards(seat.player).map(card => card.label).join('、') }}</small>
                    <small v-if="replayFrame.state.playerDecks.length">牌庫 {{ replayDeckCount(seat.player) }}</small>
                    <small v-else>共用牌庫 {{ replayFrame.state.deckCount ?? 0 }}</small>
                    <small v-if="replayPlayerDiscardCards(seat.player).length">棄牌 · {{ replayPlayerDiscardCards(seat.player).map(card => card.label).join('、') }}</small>
                  </div>
                  <span v-if="replayFrame.state.currentPlayer === seat.player" class="turn-badge">行動中</span>
                  <span class="side-hand-count">{{ replayCardsFor(seat.player).length }} 張</span>
                </div>
                <div class="hand fan seat-hand">
                  <span
                    v-for="card in replayCardsFor(seat.player)"
                    :key="card.id"
                    class="playing-card"
                    :class="[elementClass(card.element)]"
                    :aria-label="card.label"
                  >
                    <span class="card-level">{{ cardLevel(card.level) }}</span>
                    <span class="card-element">{{ cardElement(card.element) }}</span>
                  </span>
                </div>
              </div>
              <div class="board-center">
                <div class="discard-piles" aria-label="棄牌堆">
                  <div class="discard-pile"><span>棄牌</span><strong>{{ replayFrame.state.discard.length }}</strong></div>
                  <div v-for="pile in replayFrame.state.playerDiscards" :key="pile.player" class="discard-pile"><span>{{ replayPlayerLabel(pile.player) }} 棄牌</span><strong>{{ pile.cards.length }}</strong></div>
                </div>
                <div class="formation-field">
                  <div class="formation-field-heading"><span class="formation-field-label">陣法區</span><span v-if="replayFrame.state.environment" class="environment-badge">環境 · {{ environmentLabel(replayFrame.state.environment) }}</span></div>
                  <div class="previous-formation"><template v-if="replayFrame.state.previousTurnFormation"><small>上一回合 · {{ replayPlayerLabel(replayFrame.state.previousTurnFormation.player) }}</small><strong>{{ replayFrame.state.previousTurnFormation.formationName ?? '陣法' }}</strong></template><p v-else>上一回合未發動陣法</p></div>
                </div>
              </div>
              <p v-if="replayFrame.state.pendingChoice" class="action-detail">{{ replayPlayerLabel(replayFrame.state.pendingChoice.player) }} 的選擇：{{ replayFrame.state.pendingChoice.cards.map(card => card.label).join('、') }}</p>
            </section>
          </div>
          <section class="event-panel expanded"><div class="panel-title"><h2>戰局紀錄</h2></div><ol class="event-feed"><li v-for="event in replayFrame.events" :key="event.id"><div><span>{{ event.title }}</span><p>{{ event.summary }}</p></div></li></ol></section>
        </template>
        <p v-else-if="!replayLoading" class="muted">找不到這個重播</p>
      </section>
    </main>

    <main v-else-if="screen === 'lobby'" class="lobby-page">
      <div class="lobby-heading">
        <div>
          <h1>房間</h1>
          <p class="muted">選擇等待中的房間，或使用房間代碼加入。</p>
        </div>
        <div class="lobby-actions">
          <form class="room-code-form" @submit.prevent="joinRoomByCode">
            <label class="sr-only" for="join-room-code">房間代碼</label>
            <input
              id="join-room-code"
              v-model.trim="joinRoomCode"
              class="code-input"
              type="text"
              placeholder="房間代碼"
            >
            <button type="submit" :disabled="lobbyBusy || !joinRoomCode">
              加入
            </button>
          </form>
          <button
            ref="createRoomTrigger"
            class="primary-button create-room-button"
            type="button"
            @click="openRoomSettings"
          >
            建立房間
          </button>
        </div>
      </div>

      <p v-if="lobbyError && !roomSettingsOpen" class="form-error lobby-error">{{ lobbyError }}</p>

      <div
        v-if="roomSettingsOpen"
        class="room-settings-layer"
        @click.self="closeRoomSettings"
      >
        <form
          class="setup-card room-settings-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="room-settings-title"
          @submit.prevent="createRoom"
        >
          <div class="card-heading">
            <span class="step-number">+</span>
            <div>
              <h2 id="room-settings-title">建立房間</h2>
              <p>設定這場對戰的基本資訊。</p>
            </div>
          </div>

          <label for="room-name">房間名稱</label>
          <input
            id="room-name"
            ref="roomNameInput"
            v-model="roomName"
            class="text-input"
            maxlength="24"
          >

          <fieldset>
            <legend>對戰模式</legend>
            <div class="option-grid">
              <button
                v-for="mode in modes"
                :key="mode.id"
                type="button"
                class="mode-option"
                :class="{ selected: roomMode === mode.id }"
                @click="roomMode = mode.id"
              >
                <span class="mode-icon">{{ mode.icon }}</span>
                <strong>{{ mode.label }}</strong>
                <small>{{ mode.description }}</small>
              </button>
            </div>
          </fieldset>

          <fieldset>
            <legend>房間權限</legend>
            <div class="segmented">
              <button
                type="button"
                :class="{ active: roomAccess === 'private' }"
                @click="roomAccess = 'private'"
              >
                私人房間
              </button>
              <button
                type="button"
                :class="{ active: roomAccess === 'public' }"
                @click="roomAccess = 'public'"
              >
                公開房間
              </button>
            </div>
          </fieldset>

          <div class="setup-summary">
            <div>
              <span>目前設定</span>
              <strong>{{ roomModeLabel }} · {{ roomAccess === 'private' ? '私人房間' : '公開房間' }}</strong>
            </div>
            <p>{{ roomAccess === 'public' ? '公開房間會顯示於可加入清單。' : '私人房間僅能透過邀請連結或房碼加入。' }}</p>
          </div>

          <p v-if="lobbyError" class="form-error">{{ lobbyError }}</p>

          <div class="setup-actions">
            <button class="secondary-button" type="button" :disabled="lobbyBusy" @click="closeRoomSettings">
              取消
            </button>
            <button class="primary-button start-button" type="submit" :disabled="lobbyBusy">
              {{ lobbyBusy ? '處理中…' : '建立房間' }} <span>→</span>
            </button>
          </div>
        </form>
      </div>

      <section class="public-rooms-card">
        <div class="panel-title">
          <h2>公開房間</h2>
          <span>可加入</span>
        </div>
        <p v-if="!joinablePublicRooms.length" class="muted">目前沒有可加入的公開房間。</p>
        <div v-else class="public-room-list">
          <button
            v-for="room in joinablePublicRooms"
            :key="room.gameId"
            type="button"
            :disabled="lobbyBusy"
            @click="enterListedRoom(room)"
          >
            <span class="room-code">{{ room.roomCode }}</span>
            <div>
              <strong>{{ room.name }}</strong>
              <small>{{ room.members.length }} / {{ room.capacity }} 玩家 · 等待開始</small>
              <small v-if="roomRuleSummary(room)">{{ roomRuleSummary(room) }}</small>
            </div>
            <i>加入</i>
          </button>
        </div>
      </section>

      <section class="public-rooms-card my-rooms-card">
        <div class="panel-title">
          <h2>我的房間</h2>
          <span>{{ myRooms.length }}</span>
        </div>
        <p v-if="!myRooms.length" class="muted">尚未加入任何房間。</p>
        <div v-else class="public-room-list">
          <button
            v-for="room in myRooms"
            :key="`mine-${room.gameId}`"
            type="button"
            :disabled="lobbyBusy"
            @click="openJoinedRoom(room.gameId)"
          >
            <span class="room-code">{{ room.gameId.slice(0, 8) }}</span>
            <div>
              <strong>{{ room.name }}</strong>
              <small>{{ roomStatusLabel(room) }}</small>
              <small v-if="roomRuleSummary(room)">{{ roomRuleSummary(room) }}</small>
            </div>
            <i>{{ roomNeedsAttention(room) ? '輪到你' : '進入' }}</i>
          </button>
        </div>
      </section>
    </main>

    <main v-else class="game-page">
      <div class="battle-layout">
        <section
          class="battlefield"
          :class="{
            'four-player': playerSeats.length === 4,
            'discard-open': discardOpen,
          }"
          aria-label="五行戰牌對戰桌"
        >
          <button
            class="back-button battlefield-back"
            type="button"
            aria-label="返回房間列表"
            title="返回房間列表"
            @click="leaveGame"
          >
            ←
          </button>

          <div
            v-for="seat in playerSeats"
            :key="seat.player"
            class="player-seat"
            :class="[
              `seat-${seat.position}`,
              { acting: !gameFinished && state.currentPlayer === seat.player },
            ]"
          >
            <div class="player-identity">
              <span
                class="connection-dot"
                :class="{ connected: playerConnected(seat.player) }"
                :title="playerConnected(seat.player) ? '已連線' : '已斷線'"
              />
              <span class="avatar">{{ playerInitialFor(seat.player) }}</span>
              <div>
                <strong>{{ playerLabel(seat.player) }}</strong>
                <small>{{ teamHp(teamForPlayer(seat.player)) }} HP</small>
                <small v-if="teamStar(teamForPlayer(seat.player))">
                  星辰 · {{ starLabel(teamStar(teamForPlayer(seat.player))!) }}
                </small>
                <small
                  v-if="state.enabledRuleModules.includes('star') && starHistoryLabel(seat.player)"
                >
                  召星 · {{ starHistoryLabel(seat.player) }}
                </small>
                <small v-if="spiritFor(seat.player)" class="spirit-status">
                  精靈 · {{ spiritLabel(spiritFor(seat.player)!.spirit) }}
                  · 靈力 {{ spiritFor(seat.player)!.power }} / 6
                </small>
                <small v-if="state.pouches.some(pouch => pouch.owner === seat.player)">
                  錦囊 ·
                  {{ state.pouches.find(pouch => pouch.owner === seat.player)?.card?.label ?? '覆蓋牌' }}
                </small>
                <span
                  v-if="professionFor(seat.player)"
                  class="profession-badge"
                  tabindex="0"
                  :aria-label="professionSummaryLabel(seat.player)"
                >
                  職業 · {{ professionFor(seat.player)!.name }}
                  <span class="profession-summary" role="note">
                    <b>{{ professionFor(seat.player)!.name }}</b>
                    <small
                      v-for="ability in professionFor(seat.player)!.abilities"
                      :key="ability"
                    >
                      {{ ability }}
                    </small>
                  </span>
                </span>
                <small
                  v-for="(interpretation, index) in cardInterpretationsFor(seat.player)"
                  :key="`${seat.player}-interpretation-${index}`"
                  class="card-interpretation"
                >
                  {{ presentCardInterpretation(interpretation) }}
                </small>
                <small v-if="state.enabledRuleModules.includes('personal-deck')">
                  牌庫 {{ playerDeckCount(seat.player) }} · 棄牌 {{ playerDiscardCount(seat.player) }}
                </small>
                <small
                  v-for="card in exposedDeckCards(seat.player)"
                  :key="`exposed-deck-${seat.player}-${card.id}`"
                >
                  公開牌：{{ card.label }}
                </small>
              </div>
              <span
                v-if="seat.player === ownPlayer && game.connectionState.value === 'reconnecting'"
                class="reconnecting-label"
              >
                重新連線中
              </span>
              <span
                v-if="state.currentPlayer === seat.player && !roomWaiting && !gameFinished"
                class="turn-badge"
              >
                行動中 · {{ phaseLabel(state.phase) }}
              </span>
              <span
                v-for="counter in counterEffectsFor(seat.player)"
                :key="`${seat.player}-${counter.effectId}`"
                class="counter-badge"
              >
                反制 · {{ counter.effectName }}
              </span>
              <span
                v-if="shieldFor(seat.player) > 0"
                class="shield-badge"
              >
                防護罩 · {{ shieldFor(seat.player) }}
              </span>
              <span
                v-for="effect in persistentEffectsFor(seat.player)"
                :key="`${seat.player}-${effect.key}`"
                class="status-badge persistent-effect"
              >
                {{ effect.label }}
              </span>
              <span class="side-hand-count">{{ handCount(seat.player) }} 張</span>
            </div>

            <div class="hand fan seat-hand">
              <button
                v-for="card in cardsFor(seat.player)"
                :key="card.id"
                type="button"
                class="playing-card"
                :class="[
                  { hidden: card.hidden, selected: card.selected },
                  elementClass(card.element),
                ]"
                :data-card-id="card.cardId"
                :disabled="!card.selectable || !roomConnected"
                :aria-label="card.hidden ? '牌背' : card.label"
                @click="game.toggleCardSelection(seat.player, card.cardId)"
              >
                <span v-if="!card.hidden" class="card-level">{{ cardLevel(card.level) }}</span>
                <span v-if="!card.hidden" class="card-element">{{ cardElement(card.element) }}</span>
              </button>
            </div>
          </div>

          <div class="board-center">
            <div
              class="discard-piles"
              :class="{ personal: state.playerDiscards.length > 0 }"
              aria-label="棄牌堆"
            >
              <div
                v-for="pile in visibleDiscardPiles"
                :key="pile.owner ?? 'shared'"
                class="discard-pile"
                :class="[
                  { disabled: discardPileUnavailable(pile.cards) },
                  pile.position ? `discard-position-${pile.position}` : '',
                ]"
              >
                <span>{{ pile.owner ? `${playerLabel(pile.owner)} 棄牌` : '棄牌' }}</span>
                <button
                  class="discard-pile-trigger"
                  type="button"
                  aria-haspopup="dialog"
                  aria-controls="discard-composition"
                  :aria-expanded="discardOpen && activeDiscardOwner === pile.owner"
                  :aria-disabled="discardPileUnavailable(pile.cards)"
                  :aria-label="`查看${pile.owner ? `${playerLabel(pile.owner)}的` : ''}棄牌內容，共 ${pile.cards.length} 張`"
                  @click.stop="toggleDiscardComposition(pile.owner, $event)"
                >
                  {{ pile.cards.length }}
                </button>
              </div>
            </div>
            <div class="formation-field">
              <div class="formation-field-heading">
                <span class="formation-field-label">陣法區</span>
                <span v-if="state.environment" class="environment-badge" aria-live="polite">
                  環境 · {{ environmentLabel(state.environment) }}
                </span>
              </div>
              <div class="previous-formation">
                <template v-if="state.previousTurnFormation">
                  <small>上一回合 · {{ playerLabel(state.previousTurnFormation.player) }}</small>
                  <strong>{{ state.previousTurnFormation.formationName ?? '蓋牌' }}</strong>
                  <div class="formation-cards">
                    <span
                      v-for="card in previousFormationCards"
                      :key="card.id"
                      class="formation-card"
                      :class="[{
                        hidden: card.hidden,
                      }, elementClass(card.element)]"
                      :aria-label="card.hidden ? '牌背' : card.label"
                    >
                      <i v-if="!card.hidden">{{ cardLevel(card.level) }}</i>
                      <b v-if="!card.hidden">{{ cardElement(card.element) }}</b>
                    </span>
                  </div>
                </template>
                <p v-else>上一回合未發動陣法</p>
              </div>

              <div
                v-if="!roomWaiting && !gameFinished && viewer === state.currentPlayer"
                class="turn-controls"
              >
                <p v-if="game.isLoading.value" class="action-processing">處理中</p>
                <p v-else-if="game.errorMessage.value" class="action-error">
                  {{ game.errorMessage.value }}
                </p>

                <section class="ability-panel" aria-labelledby="ability-panel-title">
                  <header>
                    <h3 id="ability-panel-title">能力</h3>
                    <small>不結束行動階段</small>
                  </header>
                  <div class="action-candidates">
                    <button
                      v-for="pouchAction in pouchStrategyActions"
                      :key="`pouch-${pouchAction.label}`"
                      type="button"
                      :title="pouchAction.detail"
                      :aria-label="`${pouchAction.label}：${pouchAction.detail}`"
                      :disabled="!roomConnected || game.isLoading.value"
                      @mouseenter="showTextActionDetail(pouchAction.label, pouchAction.detail)"
                      @mouseleave="hideActionDetail"
                      @focus="showTextActionDetail(pouchAction.label, pouchAction.detail)"
                      @blur="hideActionDetail"
                      @click="startPouchAction(pouchAction)"
                    >
                      {{ pouchAction.label }}
                    </button>
                    <button
                      v-for="ability in directPlayableAbilities"
                      :key="playableAbilityKey(ability)"
                      type="button"
                      :title="playableActionDetail(ability)"
                      :aria-label="`${ability.name}：${playableActionDetail(ability)}`"
                      @mouseenter="showActionDetail(ability)"
                      @mouseleave="hideActionDetail"
                      @focus="showActionDetail(ability)"
                      @blur="hideActionDetail"
                      @click="game.performPlayableAction(ability)"
                    >
                      {{ ability.name }}
                    </button>
                    <div
                      v-if="splendorAbilities.length"
                      class="spirit-level-picker"
                      role="group"
                      aria-label="絢爛：選擇指定等級"
                      @click.stop
                    >
                      <button
                        ref="splendorMenuTrigger"
                        class="spirit-level-trigger"
                        type="button"
                        aria-haspopup="menu"
                        :aria-expanded="splendorMenuOpen"
                        @click="splendorMenuOpen = !splendorMenuOpen"
                      >
                        絢爛
                      </button>
                      <div v-if="splendorMenuOpen" class="spirit-level-options" role="menu">
                        <button
                          v-for="ability in splendorAbilities"
                          :key="playableAbilityKey(ability)"
                          type="button"
                          role="menuitem"
                          :title="playableActionDetail(ability)"
                          :aria-label="`絢爛：指定為 ${ability.declaredLevel} 級`"
                          @click="startSplendorAction(ability)"
                        >
                          {{ ability.declaredLevel }} 級
                        </button>
                      </div>
                    </div>
                    <button
                      v-if="game.interaction.value.canRetrieveDiscard"
                      class="retrieve-action"
                      type="button"
                      :title="discardRetrievalDetail"
                      :aria-label="`棄牌回收：${discardRetrievalDetail}`"
                      :disabled="!roomConnected"
                      @mouseenter="showTextActionDetail('棄牌回收', discardRetrievalDetail)"
                      @mouseleave="hideActionDetail"
                      @focus="showTextActionDetail('棄牌回收', discardRetrievalDetail)"
                      @blur="hideActionDetail"
                      @click="game.retrievePreviousTurnDiscard()"
                    >
                      棄牌回收
                    </button>
                    <p v-if="!game.playableAbilities.value.length && !game.interaction.value.canRetrieveDiscard">
                      目前沒有可用能力
                    </p>
                  </div>
                </section>

                <section class="action-panel" aria-labelledby="action-panel-title">
                  <header>
                    <h3 id="action-panel-title">行動</h3>
                    <small>使用後結束行動階段</small>
                  </header>
                  <div v-if="!game.isLoading.value" class="action-candidates">
                    <button
                      v-for="action in game.playableMainActions.value"
                      :key="`${action.type}:${action.id}:${action.cards.join('-')}:${action.type === 'performFormation' ? `${action.starSubstitution?.card ?? 'printed'}:${action.matchOption?.role ?? 'default'}:${action.matchOption?.card ?? ''}` : 'profession'}`"
                      type="button"
                      :title="playableActionDetail(action)"
                      @mouseenter="showActionDetail(action)"
                      @mouseleave="hideActionDetail"
                      @focus="showActionDetail(action)"
                      @blur="hideActionDetail"
                      @pointerdown="startActionDetail(action)"
                      @pointerup="cancelActionDetail"
                      @pointercancel="cancelActionDetail"
                      @click="startPlayableAction(action)"
                    >
                      {{ playableActionName(action) }}
                    </button>
                    <button
                      v-if="showSkip"
                      class="skip-action"
                      type="button"
                      :disabled="!roomConnected"
                      @click="game.passAction()"
                    >
                      跳過
                    </button>
                  </div>
                  <p
                    v-if="!game.isLoading.value && !game.playableMainActions.value.length && !game.errorMessage.value"
                    class="action-prompt"
                  >
                    選擇手牌以尋找可用行動
                  </p>
                </section>

                <p v-if="actionDetail" class="action-detail">
                  <strong>{{ actionDetail.name }}</strong>
                  {{ actionDetail.summary }}
                </p>
              </div>
            </div>
            <div
              v-if="discardOpen"
              class="discard-composition-layer"
            >
                <section
                  id="discard-composition"
                  class="card-composition discard-composition"
                  role="dialog"
                  aria-labelledby="discard-composition-title"
                >
                  <h2 id="discard-composition-title">棄牌內容</h2>
                  <table>
                    <caption class="sr-only">依五行與等級統計棄牌張數。列為五行，欄為等級。</caption>
                    <thead>
                      <tr>
                        <th scope="col"><span class="sr-only">五行</span></th>
                        <th
                          v-for="level in CARD_LEVELS"
                          :key="`discard-heading-${level}`"
                          scope="col"
                        >
                          {{ level }} 級
                        </th>
                      </tr>
                    </thead>
                    <tbody>
                      <tr v-for="row in discardComposition" :key="`discard-element-${row.element}`">
                        <th scope="row">{{ row.element }}</th>
                        <td
                          v-for="cell in row.cells"
                          :key="`${cell.element}-${cell.level}`"
                          :class="{ empty: cell.count === 0 }"
                        >
                          <span class="sr-only">{{ cell.element }} {{ cell.level }} 級，共 {{ cell.count }} 張</span>
                          <strong aria-hidden="true">{{ cell.count }}</strong>
                        </td>
                      </tr>
                    </tbody>
                  </table>
                </section>
            </div>
          </div>

          <div
            v-if="state.status === 'Preparing' && game.interaction.value.canChooseInitialPouch"
            class="choice-overlay"
            role="dialog"
            aria-label="選擇初始錦囊"
          >
            <div>
              <h2>選擇初始錦囊</h2>
              <p>先從個人牌組選一張牌；所有玩家完成後才洗牌發牌。</p>
              <CardChoiceMatrix
                :cards="initialPouchCards"
                :disabled="game.isLoading.value || !roomConnected"
                label="初始錦囊牌組矩陣"
                caption="依五行與等級選擇初始錦囊"
                action-label="選擇作為初始錦囊"
                @select="chooseInitialPouchCard"
              />
            </div>
          </div>

          <div
            v-else-if="state.status === 'Preparing'"
            class="choice-waiting-overlay"
            role="status"
            aria-live="polite"
          >
            <div>
              <h2>等待選擇錦囊</h2>
              <p v-if="state.preparationPlayer">
                等待 {{ playerLabel(state.preparationPlayer) }} 完成選擇。
              </p>
              <p v-else>伺服器正在洗牌與發牌。</p>
            </div>
          </div>

          <div
            v-if="pouchChoiceKind"
            class="choice-overlay"
            role="dialog"
            :aria-label="pouchChoiceKind === 'chain' ? '連環：選擇錦囊' : '牽羊：交換牌'"
          >
            <div>
              <h2>{{ pouchChoiceKind === 'chain' ? '連環：選擇牌組牌' : '牽羊：交換牌' }}</h2>
              <p v-if="pouchChoiceKind === 'chain'">
                先依五行與等級選一張作為錦囊；也可再選一張公開觸發秘計。
              </p>
              <p v-else>各選 {{ pouchSwapRequiredCount }} 張牌組牌與棄牌交換，之後洗牌。</p>

              <template v-if="pouchChoiceKind === 'chain'">
                <h3>選擇錦囊牌</h3>
                <p v-if="chainPouchCard" class="choice-selection-summary">
                  已選：{{ chainPouchCard.label }}
                  <button type="button" @click="clearChainPouchCard">重新選擇</button>
                </p>
                <CardChoiceMatrix
                  class="chain-composition"
                  :cards="chainPouchCards"
                  :selected-cards="pouchDeckSelection.slice(0, 1)"
                  label="連環錦囊牌組矩陣"
                  caption="依五行與等級選擇連環錦囊牌"
                  action-label="選擇作為連環錦囊"
                  @select="chooseChainPouchCard"
                />

                <template v-if="chainPouchCard">
                  <h3>選擇觸發牌（可選）</h3>
                  <p v-if="chainTriggerCard" class="choice-selection-summary">
                    已選：{{ chainTriggerCard.label }}
                    <button type="button" @click="clearChainTriggerCard">不觸發秘計</button>
                  </p>
                  <CardChoiceMatrix
                    class="chain-composition"
                    :cards="chainTriggerCards"
                    :selected-cards="pouchDeckSelection.slice(1, 2)"
                    label="連環觸發牌組矩陣"
                    caption="依五行與等級選擇連環觸發牌"
                    action-label="選擇作為連環觸發牌"
                    @select="chooseChainTriggerCard"
                  />
                </template>
              </template>

              <template v-else>
                <CardChoiceMatrix
                  :cards="pouchSwapSelectableDeckCards"
                  :selected-cards="pouchDeckSelection"
                  :maximum="pouchSwapRequiredCount"
                  mode="toggle"
                  label="牽羊牌組矩陣"
                  caption="依五行與等級選擇牽羊交換的牌組牌"
                  action-label="選擇牽羊牌組牌"
                  @select="togglePouchCard('pouchDeck', $event, pouchSwapRequiredCount)"
                />
                <h3>選擇 {{ pouchSwapRequiredCount }} 張要放回牌組的牌</h3>
                <CardChoiceMatrix
                  :cards="pouchSwapReturnCards"
                  :selected-cards="pouchDiscardSelection"
                  :maximum="pouchSwapRequiredCount"
                  mode="toggle"
                  label="牽羊回收矩陣"
                  caption="依五行與等級選擇牽羊放回牌組的牌"
                  action-label="選擇牽羊回收牌"
                  @select="togglePouchCard('pouchDiscard', $event, pouchSwapRequiredCount)"
                />
              </template>

              <template v-if="pouchChoiceKind === 'chain'">
                <h3>錦囊持有者</h3>
                <div class="choice-options" aria-label="選擇錦囊持有者">
                  <button
                    v-for="player in friendlyPouchOwners"
                    :key="`pouch-owner-${player}`"
                    type="button"
                    :class="{ selected: pouchOwnerSelection === player }"
                    :aria-pressed="pouchOwnerSelection === player"
                    @click="pouchOwnerSelection = player"
                  >
                    {{ playerLabel(player) }}
                  </button>
                </div>

                <template v-if="chainTriggerCard">
                  <h3>觸發秘計</h3>
                  <div class="choice-options" aria-label="選擇秘計">
                    <button
                      v-for="option in chainStrategyOptions"
                      :key="`chain-strategy-${option.strategy}`"
                      type="button"
                      :class="{ selected: chainStrategySelection === option.strategy }"
                      :aria-pressed="chainStrategySelection === option.strategy"
                      @click="chainStrategySelection = option.strategy"
                    >
                      {{ strategyLabel(option.strategy) }}
                    </button>
                  </div>

                  <div
                    v-if="selectedChainStrategyAction?.input === 'targetPlayer'"
                    class="choice-options"
                    aria-label="離山目標"
                  >
                    <button
                      v-for="player in selectedChainStrategyAction.targetPlayers"
                      :key="`strategy-target-${player}`"
                      type="button"
                      :class="{ selected: strategyTargetSelection === player }"
                      :aria-pressed="strategyTargetSelection === player"
                      @click="strategyTargetSelection = player"
                    >
                      {{ playerLabel(player) }}
                    </button>
                  </div>

                  <template v-if="selectedChainStrategyAction?.input === 'deckDiscardSwap'">
                    <h3>牽羊：另選兩張牌組牌</h3>
                    <CardChoiceMatrix
                      :cards="chainStrategySelectableDeckCards"
                      :selected-cards="strategyDeckSelection"
                      :maximum="selectedChainStrategyAction?.requiredCardCount ?? 0"
                      mode="toggle"
                      label="連環牽羊牌組矩陣"
                      caption="依五行與等級選擇連環牽羊交換的牌組牌"
                      action-label="選擇連環牽羊牌組牌"
                      @select="togglePouchCard(
                        'strategyDeck', $event, selectedChainStrategyAction?.requiredCardCount ?? 0,
                      )"
                    />
                    <h3>牽羊：選兩張要放回牌組的牌</h3>
                    <CardChoiceMatrix
                      :cards="chainStrategyReturnCards"
                      :selected-cards="strategyDiscardSelection"
                      :maximum="selectedChainStrategyAction?.requiredCardCount ?? 0"
                      mode="toggle"
                      label="連環牽羊回收矩陣"
                      caption="依五行與等級選擇連環牽羊放回牌組的牌"
                      action-label="選擇連環牽羊回收牌"
                      @select="togglePouchCard(
                        'strategyDiscard', $event, selectedChainStrategyAction?.requiredCardCount ?? 0,
                      )"
                    />
                  </template>

                  <template v-if="selectedChainStrategyAction?.input === 'star'">
                    <h3>瞞天：取得星辰效果或破除星辰</h3>
                    <div class="choice-options" aria-label="瞞天選擇">
                      <button
                        v-for="star in selectedChainStrategyAction.stars"
                        :key="`strategy-star-${star}`"
                        type="button"
                        :class="{ selected: strategyStarSelection === star && !strategyBreakStar }"
                        :aria-pressed="strategyStarSelection === star && !strategyBreakStar"
                        @click="strategyStarSelection = star; strategyBreakStar = false"
                      >
                        取得 {{ starLabel(star) }}
                      </button>
                      <button
                        v-for="star in selectedChainStrategyAction.breakStars"
                        :key="`strategy-break-star-${star}`"
                        type="button"
                        :class="{ selected: strategyStarSelection === star && strategyBreakStar }"
                        :aria-pressed="strategyStarSelection === star && strategyBreakStar"
                        @click="strategyStarSelection = star; strategyBreakStar = true"
                      >
                        破除 {{ starLabel(star) }}
                      </button>
                    </div>
                  </template>

                  <template v-if="selectedChainStrategyAction?.input === 'retreat'">
                    <h3>走為：破除環境或捨棄手牌</h3>
                    <div class="choice-options" aria-label="走為選擇">
                      <button
                        type="button"
                        :class="{ selected: strategyDiscardCard === null }"
                        :aria-pressed="strategyDiscardCard === null"
                        @click="strategyDiscardCard = null"
                      >
                        破除環境
                      </button>
                      <button
                        v-for="card in ownHandCards.filter(
                          candidate => selectedChainStrategyAction?.handCards.includes(candidate.id),
                        )"
                        :key="`strategy-hand-${card.id}`"
                        type="button"
                        :class="{ selected: strategyDiscardCard === card.id }"
                        :aria-pressed="strategyDiscardCard === card.id"
                        @click="strategyDiscardCard = card.id"
                      >
                        捨棄 {{ card.label }}
                      </button>
                    </div>
                  </template>
                </template>
              </template>

              <div class="choice-options choice-actions">
                <button
                  class="choice-confirm"
                  type="button"
                  :disabled="!canSubmitPouchChoice || game.isLoading.value || !roomConnected"
                  @click="submitPouchChoice"
                >
                  確認
                </button>
                <button type="button" @click="resetPouchChoice">取消</button>
              </div>
            </div>
          </div>

          <div
            v-if="state.pendingChoice && viewer === state.pendingChoice.player"
            class="choice-overlay"
          >
            <div>
              <h2>{{ pendingChoiceLabel(state.pendingChoice) }}</h2>
              <CardChoiceMatrix
                v-if="state.pendingChoice.presentation.type === 'echoRingingMetalDeckCard'"
                :cards="state.pendingChoice.cards"
                :selected-cards="game.selectedChoiceCards.value"
                :maximum="state.pendingChoice.maximumCount"
                :disabled="game.isLoading.value || !roomConnected"
                mode="toggle"
                label="商調‧鳴金牌組矩陣"
                caption="依五行與等級選擇商調‧鳴金檢索的牌組牌"
                action-label="選擇商調‧鳴金牌組牌"
                @select="game.togglePendingChoiceCard"
              />
              <div v-else class="choice-cards">
                <button
                  v-for="card in state.pendingChoice.cards"
                  :key="card.id"
                  type="button"
                  :class="{ selected: game.selectedChoiceCards.value.includes(card.id) }"
                  :aria-pressed="state.pendingChoice.kind === 'EffectGenerated'
                    || state.pendingChoice.kind === 'TypedEffect'
                    ? game.selectedChoiceCards.value.includes(card.id)
                    : undefined"
                  :disabled="
                    game.isLoading.value
                    || !roomConnected
                    || viewer !== state.pendingChoice.player
                    || (
                      (state.pendingChoice.kind === 'EffectGenerated'
                        || state.pendingChoice.kind === 'TypedEffect')
                      && game.selectedChoiceCards.value.length >= state.pendingChoice.maximumCount
                      && !game.selectedChoiceCards.value.includes(card.id)
                    )
                  "
                  @click="state.pendingChoice.kind === 'EffectGenerated'
                    || state.pendingChoice.kind === 'TypedEffect'
                    ? game.togglePendingChoiceCard(card.id)
                    : game.choosePendingCard(card.id)"
                >
                  {{ card.label }}
                </button>
              </div>
              <template
                v-if="(state.pendingChoice.kind === 'EffectGenerated'
                  || state.pendingChoice.kind === 'TypedEffect')
                  && state.pendingChoice.cards.length > 0
                  && viewer === state.pendingChoice.player"
              >
                <p class="choice-count">
                  已選 {{ game.selectedChoiceCards.value.length }}
                  （{{ state.pendingChoice.minimumCount }}–{{ state.pendingChoice.maximumCount }}）
                </p>
                <button
                  class="choice-submit"
                  type="button"
                  :disabled="game.isLoading.value || !roomConnected || !game.canSubmitPendingChoice.value"
                  @click="game.submitPendingChoice()"
                >
                  確認選擇
                </button>
              </template>
              <div
                v-if="state.pendingChoice.kind === 'TypedEffect'
                  && state.pendingChoice.players.length > 0"
                class="choice-options"
                aria-label="選擇玩家"
              >
                <button
                  v-for="player in state.pendingChoice.players"
                  :key="`choice-player-${player}`"
                  type="button"
                  :disabled="game.isLoading.value || !roomConnected"
                  :aria-label="`選擇玩家 ${playerLabel(player)}`"
                  @click="game.choosePendingPlayer(player)"
                >
                  {{ playerLabel(player) }}
                </button>
              </div>
              <SplitEarthFormationChoice
                v-if="state.pendingChoice.kind === 'TypedEffect'
                  && usesSplitEarthFormationGroups(state.pendingChoice)"
                :key="splitEarthChoiceKey(
                  state.pendingChoice,
                  state.turnNumber,
                  roomConnected,
                )"
                :groups="state.pendingChoice.formationGroups"
                :disabled="game.isLoading.value || !roomConnected"
                @select="game.choosePendingFormation"
              />
              <div
                v-else-if="state.pendingChoice.kind === 'TypedEffect'
                  && state.pendingChoice.formations.length > 0"
                class="choice-options"
                aria-label="選擇陣法"
              >
                <button
                  v-for="formationId in state.pendingChoice.formations"
                  :key="`choice-formation-${formationId}`"
                  type="button"
                  :disabled="game.isLoading.value || !roomConnected"
                  :aria-label="`選擇陣法 ${formationChoiceLabel(formationId)}`"
                  @click="game.choosePendingFormation(formationId)"
                >
                  {{ formationChoiceLabel(formationId) }}
                </button>
              </div>
              <div
                v-if="state.pendingChoice.kind === 'TypedEffect'
                  && state.pendingChoice.environments.length > 0"
                class="choice-options"
                aria-label="選擇環境"
              >
                <button
                  v-for="environment in state.pendingChoice.environments"
                  :key="`choice-environment-${environment}`"
                  type="button"
                  :disabled="game.isLoading.value || !roomConnected"
                  :aria-label="`選擇環境 ${environmentLabel(environment)}`"
                  @click="game.choosePendingEnvironment(environment)"
                >
                  {{ environmentLabel(environment) }}
                </button>
              </div>
              <button
                v-if="state.pendingChoice.kind === 'TypedEffect'
                  && state.pendingChoice.canDecline"
                class="choice-submit"
                type="button"
                :disabled="game.isLoading.value || !roomConnected"
                @click="game.declinePendingChoice()"
              >
                放棄迴響
              </button>
            </div>
          </div>

          <div
            v-else-if="state.pendingChoice"
            class="choice-waiting-overlay"
            role="status"
            aria-live="polite"
          >
            <div>
              <h2>等待 {{ playerLabel(state.pendingChoice.player) }}</h2>
              <p>{{ pendingChoiceLabel(state.pendingChoice) }}</p>
            </div>
          </div>

          <div
            v-if="state.pendingRandomness"
            class="choice-overlay"
            role="status"
            aria-live="polite"
          >
            <div>
              <h2>伺服器正在洗牌</h2>
              <p>正在安全地重新排列 {{ state.pendingRandomness.cardCount }} 張牌。</p>
            </div>
          </div>

          <div v-if="showSetupReveal" class="setup-reveal">
            <div>
              <h2>{{ onlineMetadata?.capacity === 4 ? '隊伍與行動順序' : '行動順序' }}</h2>
              <div v-if="onlineMetadata?.capacity === 4" class="revealed-teams">
                <span v-for="team in activeTeams" :key="team">
                  <strong>{{ teamLabel(team) }}</strong>
                  {{ teamMembers(team).join('、') }}
                </span>
              </div>
              <ol>
                <li v-for="player in state.turnOrder" :key="player">{{ playerLabel(player) }}</li>
              </ol>
            </div>
          </div>

          <div v-if="roomWaiting" class="waiting-overlay">
            <div class="waiting-panel">
              <section class="waiting-main">
                <h2>{{ activeRoomName }}</h2>
                <p>{{ waitingRoomSummary }}</p>
                <p v-if="game.lockedDeckName.value" class="muted">
                  本局使用：{{ game.lockedDeckName.value }}
                </p>
                <fieldset class="waiting-rules">
                  <legend>{{ isRoomOwner ? '規則模組' : '啟用規則' }}</legend>
                  <section
                    v-for="group in ruleGroups"
                    :key="group.id"
                    class="waiting-rule-group"
                    :aria-labelledby="`waiting-rule-group-${group.id}`"
                  >
                    <h3 :id="`waiting-rule-group-${group.id}`">{{ group.label }}</h3>
                    <label v-for="rule in group.rules" :key="rule.id" class="rule-toggle">
                      <input
                        type="checkbox"
                        :checked="onlineMetadata?.enabledRuleModules.includes(rule.id)"
                        :disabled="game.isLoading.value || !isRoomOwner"
                        @change="toggleWaitingRule(rule.id)"
                      >
                      {{ rule.label }}
                    </label>
                  </section>
                </fieldset>
                <p v-if="game.errorMessage.value" class="form-error" role="alert">
                  {{ game.errorMessage.value }}
                </p>
              </section>

              <aside class="waiting-side" aria-label="房間成員與操作">
                <div class="waiting-members">
                  <span
                    v-for="player in onlinePlayers"
                    :key="player"
                    :class="{ joined: Boolean(memberForPlayer(player)) }"
                  >
                    {{ playerLabel(player) }}
                    <small>{{ waitingMemberStatus(memberForPlayer(player)) }}</small>
                    <button
                      v-if="isRoomOwner && memberForPlayer(player) && !memberForPlayer(player)?.owner"
                      type="button"
                      :disabled="game.isLoading.value"
                      @click="removeWaitingPlayer(memberForPlayer(player)!.userId)"
                    >
                      移除
                    </button>
                  </span>
                </div>
                <div class="result-actions">
                  <button
                    v-if="isRoomOwner"
                    class="ghost-button"
                    type="button"
                    :disabled="game.isLoading.value"
                    @click="dissolveWaitingRoom"
                  >
                    解散
                  </button>
                  <button
                    v-else
                    class="ghost-button"
                    type="button"
                    :disabled="game.isLoading.value"
                    @click="leaveWaitingRoom"
                  >
                    離開
                  </button>
                  <button
                    v-if="isRoomOwner"
                    class="primary-button"
                    type="button"
                    :disabled="game.isLoading.value || !canStartOnlineRoom"
                    @click="startOnlineRoom"
                  >
                    開始遊戲 <span>→</span>
                  </button>
                  <button
                    v-else
                    class="primary-button"
                    type="button"
                    :disabled="game.isLoading.value || !roomConnected"
                    @click="game.toggleReady()"
                  >
                    {{ currentMember?.ready ? '取消準備' : '準備' }} <span>→</span>
                  </button>
                </div>
                <button
                  v-if="onlineMetadata?.access === 'private' && isRoomOwner && game.invitation.value"
                  class="invite-link"
                  type="button"
                  @click="copyInviteLink"
                >
                  複製邀請連結
                </button>
              </aside>
            </div>
          </div>

        </section>

        <aside class="game-sidebar" :class="{ finished: gameFinished }">
          <section v-if="gameFinished" class="result-panel">
            <h2>{{ gameResultText }}</h2>
            <p>{{ firstTurnText }}，本局已結束。</p>
            <div class="result-actions">
              <button class="primary-button" type="button" :disabled="game.isLoading.value" @click="restartGame">
                返回房間 <span>→</span>
              </button>
            </div>
          </section>

          <section class="event-panel" :class="{ expanded: eventExpanded }">
            <div class="panel-title">
              <h2>戰局紀錄 <button v-if="roomWaiting && game.savableReplay.value" class="ghost-button" type="button" :disabled="replaySaving" @click="saveCurrentReplay">{{ replaySaved ? '已儲存' : '儲存本局' }}</button></h2>
              <button type="button" @click="eventExpanded = !eventExpanded">
                {{ eventExpanded ? '收合' : '完整紀錄' }}
              </button>
            </div>
            <ol class="event-feed">
              <li v-for="event in visibleEvents" :key="event.id">
                <i />
                <div
                  :role="event.eventType === 'EnabledRules' ? 'region' : undefined"
                  :aria-label="event.eventType === 'EnabledRules' ? '啟用規則' : undefined"
                >
                  <span>{{ event.title }}</span>
                  <p>{{ event.summary }}</p>
                </div>
              </li>
            </ol>
          </section>
        </aside>
      </div>
    </main>

    <aside v-if="visibleNotifications.length" class="notification-stack" aria-label="即時通知">
      <article
        v-for="notification in visibleNotifications"
        :key="notification.id"
        class="notification-item"
      >
        <button
          class="notification-main"
          type="button"
          @click="openNotification(notification.gameId, notification.id)"
        >
          <strong>{{ notification.message }}</strong>
          <span>進入房間</span>
        </button>
        <button
          class="notification-dismiss"
          type="button"
          aria-label="關閉通知"
          @click="notifications.dismiss(notification.id)"
        >
          ×
        </button>
      </article>
    </aside>
  </div>
</template>

<script setup lang="ts">
import type {
  CardInstanceId,
  Element,
  PlayableAction,
  PlayerId,
  PublicCard,
  PublicCardRefs,
  PublicGameEvent,
  PublicGameState,
  RulesCatalog,
  SecretStrategy,
  SpiritKind,
  StarKind,
  TeamId,
  ViewerId,
} from '~/types/fewfc'
import type { GameRoomMember, GameRoomResponse } from '../shared/game-room'
import {
  createRuleModulePolicy,
  presentationForRuleModule,
} from '#shared/utils/rule-modules'
import { authClient } from '~/lib/auth-client'
import { buildCardComposition, CARD_LEVELS } from '~/lib/card-composition'
import { cardElementClass, cardElementGlyph } from '~/lib/card-face-presentation'
import { presentCardInterpretation } from '~/lib/card-interpretation-presentation'
import { presentPendingChoice } from '~/lib/pending-choice-presentation'
import { presentPersistentEffects } from '~/lib/persistent-effect-presentation'
import {
  presentDiscardRetrievalAction,
  presentPlayableAction,
  presentSecretStrategyAction,
} from '~/lib/action-detail-presentation'
import { presentRoomRuleDifferences } from '#shared/utils/ruleset-presentation'
import {
  splitEarthChoiceKey,
  usesSplitEarthFormationGroups,
} from '#shared/utils/split-earth-formation-choice'
import { roomRouteResult, safeInternalPath } from '~/lib/navigation'
import {
  createDeckCompositionPolicy,
  parsePlayerDeckCounts,
  serializePlayerDeckCounts,
  type PlayerDeckList,
} from '~/lib/player-deck'
import { presentApiError } from '~/lib/api-error-presentation'
import { isLegalChainTrigger, sheepReturnCards } from '~/lib/pouch-choice'
import type { LocalPasswordResetResult } from '#shared/local-password-reset'

type Screen = 'login' | 'password-reset' | 'lobby' | 'deck' | 'game' | 'replays' | 'replay'

interface ReplaySummary {
  replayId: string
  sourceGameId: string
  roomName: string
  players: Array<{ player: string; displayName: string }>
  result: unknown
  finishedAt: string
  savedAt: string
}
interface ReplayFrame {
  currentStep: number
  totalSteps: number
  state: PublicGameState
  events: PublicGameEvent[]
  players: Array<{ player: PlayerId; displayName: string }>
  firstPlayer: PlayerId
}

function replayResultText(result: unknown): string {
  if (!result || typeof result !== 'object') return '結果未知'
  const status = (result as { status?: unknown }).status
  return status === 'Finished' ? '已完成' : '結果已記錄'
}

interface PublicRoomSummary {
  gameId: string
  roomCode: string
  name: string
  access: 'private' | 'public'
  status: string
  ownerUserId: string
  players: PlayerId[]
  members: GameRoomMember[]
  capacity: number
  enabledRuleModules: string[]
  createdAt: string
  updatedAt: string
}

const route = useRoute()
const router = useRouter()
const rulesCatalog = ref<RulesCatalog | null>(null)
const ruleModulePolicy = computed(() => createRuleModulePolicy(
  rulesCatalog.value?.ruleModules ?? [],
))
const screen = computed<Screen>(() => {
  if (route.path === '/login') return 'login'
  if (route.path === '/reset-password') return 'password-reset'
  if (route.path === '/deck') return 'deck'
  if (route.path === '/replays') return 'replays'
  if (route.path.startsWith('/replays/')) return 'replay'
  if (route.path.startsWith('/rooms/')) return 'game'
  return 'lobby'
})
const isAuthenticationScreen = computed(() => (
  screen.value === 'login' || screen.value === 'password-reset'
))
const routeReady = ref(false)
const roomRouteError = ref('')
const displayName = ref('玩家')
const nameInput = ref('')
const emailInput = ref('')
const passwordInput = ref('')
const authMode = ref<'sign-in' | 'sign-up'>('sign-in')
const authBusy = ref(false)
const loginError = ref('')
const localPasswordResetEnabled = ref(false)
const resetPasswordEmail = ref('')
const resetPasswordInput = ref('')
const resetPasswordConfirmation = ref('')
const resetPasswordBusy = ref(false)
const resetPasswordError = ref('')
const currentUserId = ref('')
const replays = ref<ReplaySummary[]>([])
const replayFrame = ref<ReplayFrame | null>(null)
const replayLoading = ref(false)
const replaySaving = ref(false)
const replaySaved = ref(false)
const replayError = ref('')
const replayReplaceSource = ref<string | null>(null)
const replayRouteId = computed(() => typeof route.params.replayId === 'string' ? route.params.replayId : '')
const profileOpen = ref(false)
const profileMenuContainer = ref<HTMLElement | null>(null)
const roomSettingsOpen = ref(false)
const createRoomTrigger = ref<HTMLButtonElement | null>(null)
const roomNameInput = ref<HTMLInputElement | null>(null)
const roomName = ref('')
const roomMode = ref('duel')
const roomCapacity = computed<2 | 4>(() => roomMode.value === 'team' ? 4 : 2)
const roomAccess = ref<'private' | 'public'>('public')
const roomCode = ref('')
const joinRoomCode = ref('')
const lobbyBusy = ref(false)
const lobbyError = ref('')
const publicRooms = ref<PublicRoomSummary[]>([])
const myRooms = ref<PublicRoomSummary[]>([])
const joinablePublicRooms = computed(() => publicRooms.value.filter(
  room => !room.members.some(member => member.userId === currentUserId.value),
))
const activeRoomName = ref('')
const viewer = ref<ViewerId>('observer')
const game = useGameRoom(viewer)
const notifications = usePlayerNotifications()
const state = game.state
const splendorAbilities = computed(() => (
  game.playableAbilities.value
    .filter(ability => ability.type === 'useSpiritSkill'
      && ability.id === 'Splendor'
      && ability.declaredLevel !== null)
    .sort((left, right) => (left.declaredLevel ?? 0) - (right.declaredLevel ?? 0))
))
const directPlayableAbilities = computed(() => (
  game.playableAbilities.value.filter(
    ability => ability.type !== 'useSpiritSkill' || ability.id !== 'Splendor',
  )
))
const initialPouchCards = computed<PublicCard[]>(() => {
  if (viewer.value === 'observer') return []
  const cards = state.value.playerDecks.find(entry => entry.player === viewer.value)?.cards
  return cards?.kind === 'known' ? cards.cards : []
})

function chooseInitialPouchCard(card: CardInstanceId | undefined) {
  if (card !== undefined) game.chooseInitialPouch(card)
}
type PouchStrategyAction = {
  label: string
  detail: string
  strategy: SecretStrategy
  options?: Parameters<typeof game.triggerSecretStrategy>[1]
  choice?: 'sheep'
  requiredCardCount?: number
  deckCards?: number[]
  discardCards?: number[]
}
const pouchStrategyActions = computed<PouchStrategyAction[]>(() => {
  if (viewer.value === 'observer' || !game.interaction.value.canTriggerPouch) return []
  const pouch = state.value.pouches.find(entry => entry.owner === viewer.value)?.card
  if (!pouch) return []
  const actions: PouchStrategyAction[] = []
  const requirements = game.interaction.value.secretStrategyActions
    .filter(requirement => requirement.sourceCard === pouch.id)
  for (const requirement of requirements) {
    const { strategy } = requirement
    const detail = presentSecretStrategyAction(requirement)
    if (requirement.input === 'none') {
      actions.push({ label: `秘計‧${strategyLabel(strategy)}`, detail, strategy })
    } else if (requirement.input === 'targetPlayer') {
      for (const player of requirement.targetPlayers) {
        actions.push({
          label: `秘計‧${strategyLabel(strategy)} → ${playerLabel(player)}`,
          detail,
          strategy,
          options: { targetPlayer: player },
        })
      }
    } else if (requirement.input === 'deckDiscardSwap') {
      if (requirement.deckCards.length >= requirement.requiredCardCount
        && requirement.discardCards.length >= requirement.requiredCardCount) {
        actions.push({
          label: `秘計‧${strategyLabel(strategy)}`,
          detail,
          strategy,
          choice: 'sheep',
          requiredCardCount: requirement.requiredCardCount,
          deckCards: requirement.deckCards,
          discardCards: requirement.discardCards,
        })
      }
    } else if (requirement.input === 'star') {
      for (const star of requirement.stars) {
        actions.push({
          label: `秘計‧${strategyLabel(strategy)} → ${starLabel(star)}`,
          detail,
          strategy,
          options: { star },
        })
      }
      for (const star of requirement.breakStars) {
        actions.push({
          label: `秘計‧${strategyLabel(strategy)} → 破除 ${starLabel(star)}`,
          detail,
          strategy,
          options: { star, breakStar: true },
        })
      }
    } else if (requirement.input === 'retreat') {
      actions.push({ label: `秘計‧${strategyLabel(strategy)} → 破除環境`, detail, strategy })
      for (const cardId of requirement.handCards) {
        const card = ownHandCards.value.find(candidate => candidate.id === cardId)
        if (card) {
          actions.push({
            label: `秘計‧${strategyLabel(strategy)} → 捨棄 ${card.label}`,
            detail,
            strategy,
            options: { discardCard: card.id },
          })
        }
      }
    }
  }
  return actions
})
const pouchChoiceKind = ref<'chain' | 'sheep' | null>(null)
const pouchFormationAction = ref<PlayableAction | null>(null)
const pouchDeckSelection = ref<number[]>([])
const pouchDiscardSelection = ref<number[]>([])
const strategyDeckSelection = ref<number[]>([])
const strategyDiscardSelection = ref<number[]>([])
const pouchOwnerSelection = ref<PlayerId | null>(null)
const chainStrategySelection = ref<SecretStrategy | null>(null)
const strategyTargetSelection = ref<PlayerId | null>(null)
const strategyStarSelection = ref<StarKind | null>(null)
const strategyBreakStar = ref(false)
const strategyDiscardCard = ref<number | null>(null)
const pouchSwapRequiredCount = ref(0)
const pouchSwapDeckCards = ref<number[]>([])
const pouchSwapDiscardCards = ref<number[]>([])
const ownDiscardCards = computed(() => {
  if (viewer.value === 'observer') return []
  return state.value.playerDiscards.find(entry => entry.player === viewer.value)?.cards ?? []
})
const ownHandCards = computed(() => {
  if (viewer.value === 'observer') return []
  const hand = state.value.hands.find(entry => entry.player === viewer.value)?.cards
  return hand?.kind === 'known' ? hand.cards : []
})
const friendlyPouchOwners = computed(() => {
  if (viewer.value === 'observer') return []
  const team = state.value.players.find(player => player.id === viewer.value)?.team
  return state.value.players.filter(player => player.team === team).map(player => player.id)
})
const chainPouchCard = computed(() => (
  pouchChoiceKind.value === 'chain' && pouchDeckSelection.value.length >= 1
    ? initialPouchCards.value.find(card => card.id === pouchDeckSelection.value[0]) ?? null
    : null
))
const chainTriggerCard = computed(() => (
  pouchChoiceKind.value === 'chain' && pouchDeckSelection.value.length === 2
    ? initialPouchCards.value.find(card => card.id === pouchDeckSelection.value[1]) ?? null
    : null
))
const chainPouchCards = computed(() => (
  initialPouchCards.value.filter(card => !strategyDeckSelection.value.includes(card.id))
))
const chainTriggerCards = computed(() => (
  initialPouchCards.value.filter(card => (
    isLegalChainTrigger(chainPouchCard.value, card)
    && !strategyDeckSelection.value.includes(card.id)
  ))
))
const pouchSwapSelectableDeckCards = computed(() => initialPouchCards.value.filter(
  card => pouchSwapDeckCards.value.includes(card.id),
))
const pouchSwapReturnCards = computed(() => sheepReturnCards(
  pouchSwapDiscardCards.value,
  ownDiscardCards.value,
  initialPouchCards.value,
  pouchDeckSelection.value,
))
const chainStrategyOptions = computed(() => {
  const card = chainTriggerCard.value
  if (!card) return []
  return game.interaction.value.secretStrategyActions
    .filter(option => option.sourceCard === card.id)
})
const selectedChainStrategyAction = computed(() => chainStrategyOptions.value.find(
  option => option.strategy === chainStrategySelection.value,
) ?? null)
const chainStrategySelectableDeckCards = computed(() => initialPouchCards.value.filter(
  card => selectedChainStrategyAction.value?.deckCards.includes(card.id)
    && !pouchDeckSelection.value.includes(card.id),
))
const chainStrategyReturnCards = computed(() => sheepReturnCards(
  selectedChainStrategyAction.value?.discardCards ?? [],
  ownDiscardCards.value,
  initialPouchCards.value,
  strategyDeckSelection.value,
))
const canSubmitPouchChoice = computed(() => {
  if (pouchChoiceKind.value === 'sheep') {
    const count = pouchSwapRequiredCount.value
    return count > 0
      && pouchDeckSelection.value.length === count
      && pouchDiscardSelection.value.length === count
      && containsSelectedCards(pouchSwapSelectableDeckCards.value, pouchDeckSelection.value)
      && containsSelectedCards(pouchSwapReturnCards.value, pouchDiscardSelection.value)
  }
  if (pouchChoiceKind.value !== 'chain'
    || !pouchFormationAction.value
    || !pouchOwnerSelection.value
    || pouchDeckSelection.value.length < 1
    || pouchDeckSelection.value.length > 2) return false
  if (pouchDeckSelection.value.length === 1) return true
  const requirement = selectedChainStrategyAction.value
  if (!requirement) return false
  if (requirement.input === 'targetPlayer') return !!strategyTargetSelection.value
  if (requirement.input === 'star') return !!strategyStarSelection.value
  if (requirement.input === 'deckDiscardSwap') {
    return strategyDeckSelection.value.length === requirement.requiredCardCount
      && strategyDiscardSelection.value.length === requirement.requiredCardCount
      && containsSelectedCards(chainStrategySelectableDeckCards.value, strategyDeckSelection.value)
      && containsSelectedCards(chainStrategyReturnCards.value, strategyDiscardSelection.value)
  }
  return true
})

function containsSelectedCards(cards: PublicCard[], selected: CardInstanceId[]): boolean {
  const candidateIds = new Set(cards.map(card => card.id))
  return selected.every(card => candidateIds.has(card))
}

function resetPouchChoice() {
  pouchChoiceKind.value = null
  pouchFormationAction.value = null
  pouchDeckSelection.value = []
  pouchDiscardSelection.value = []
  strategyDeckSelection.value = []
  strategyDiscardSelection.value = []
  pouchOwnerSelection.value = null
  chainStrategySelection.value = null
  strategyTargetSelection.value = null
  strategyStarSelection.value = null
  strategyBreakStar.value = false
  strategyDiscardCard.value = null
  pouchSwapRequiredCount.value = 0
  pouchSwapDeckCards.value = []
  pouchSwapDiscardCards.value = []
}

function togglePouchCard(
  kind: 'pouchDeck' | 'pouchDiscard' | 'strategyDeck' | 'strategyDiscard',
  card: number,
  maximum: number,
) {
  const selection = {
    pouchDeck: pouchDeckSelection,
    pouchDiscard: pouchDiscardSelection,
    strategyDeck: strategyDeckSelection,
    strategyDiscard: strategyDiscardSelection,
  }[kind]
  selection.value = selection.value.includes(card)
    ? selection.value.filter(selected => selected !== card)
    : selection.value.length < maximum ? [...selection.value, card] : selection.value

  if (kind === 'pouchDeck') {
    const available = new Set(pouchSwapReturnCards.value.map(candidate => candidate.id))
    pouchDiscardSelection.value = pouchDiscardSelection.value.filter(id => available.has(id))
  } else if (kind === 'strategyDeck') {
    const available = new Set(chainStrategyReturnCards.value.map(candidate => candidate.id))
    strategyDiscardSelection.value = strategyDiscardSelection.value.filter(id => available.has(id))
  }
}

function chooseChainPouchCard(card: CardInstanceId) {
  pouchDeckSelection.value = [card]
  resetChainStrategyChoice()
}

function chooseChainTriggerCard(card: CardInstanceId) {
  const pouch = pouchDeckSelection.value[0]
  if (pouch === undefined) return

  pouchDeckSelection.value = [pouch, card]
  resetChainStrategyChoice()
}

function clearChainPouchCard() {
  pouchDeckSelection.value = []
  resetChainStrategyChoice()
}

function clearChainTriggerCard() {
  pouchDeckSelection.value = pouchDeckSelection.value.slice(0, 1)
  resetChainStrategyChoice()
}

function resetChainStrategyChoice() {
  strategyDeckSelection.value = []
  strategyDiscardSelection.value = []
  chainStrategySelection.value = null
  strategyTargetSelection.value = null
  strategyStarSelection.value = null
  strategyBreakStar.value = false
  strategyDiscardCard.value = null
}

function startPouchAction(action: PouchStrategyAction) {
  if (action.choice === 'sheep') {
    resetPouchChoice()
    pouchChoiceKind.value = 'sheep'
    pouchSwapRequiredCount.value = action.requiredCardCount ?? 0
    pouchSwapDeckCards.value = action.deckCards ?? []
    pouchSwapDiscardCards.value = action.discardCards ?? []
    return
  }
  void game.triggerSecretStrategy(action.strategy, action.options)
}

function startPlayableAction(action: PlayableAction) {
  if (action.type === 'performFormation' && action.id === 'pouch:chain') {
    resetPouchChoice()
    pouchChoiceKind.value = 'chain'
    pouchFormationAction.value = action
    pouchOwnerSelection.value = viewer.value === 'observer' ? null : viewer.value
    return
  }
  void game.performPlayableAction(action)
}

async function submitPouchChoice() {
  if (!canSubmitPouchChoice.value) return
  if (pouchChoiceKind.value === 'sheep') {
    const submitted = await game.triggerSecretStrategy('SheepStealing', {
      deckCards: pouchDeckSelection.value,
      discardCards: pouchDiscardSelection.value,
    })
    if (submitted !== false) resetPouchChoice()
    return
  }
  const action = pouchFormationAction.value
  const owner = pouchOwnerSelection.value
  if (!action || !owner) return
  const submitted = await game.performPlayableAction(action, {
    pouchOwner: owner,
    pouchCard: pouchDeckSelection.value[0],
    triggerCard: pouchDeckSelection.value[1],
    secretStrategy: chainStrategySelection.value ?? undefined,
    secretStrategyTargetPlayer: strategyTargetSelection.value ?? undefined,
    secretStrategyStar: strategyStarSelection.value ?? undefined,
    secretStrategyBreakStar: strategyBreakStar.value,
    secretStrategyDiscardCard: strategyDiscardCard.value ?? undefined,
    secretStrategyDeckCards: strategyDeckSelection.value,
    secretStrategyDiscardCards: strategyDiscardSelection.value,
  })
  if (submitted) resetPouchChoice()
}
const actionDetail = ref<{ name: string; summary: string } | null>(null)
const discardRetrievalDetail = computed(() => {
  const detail = game.interaction.value.discardRetrievalAction
  return detail ? presentDiscardRetrievalAction(detail, playerLabel) : ''
})
const splendorMenuOpen = ref(false)
const splendorMenuTrigger = ref<HTMLButtonElement | null>(null)
const eventExpanded = ref(false)
const showSetupReveal = ref(false)
const discardOpen = ref(false)
const activeDiscardOwner = ref<PlayerId | null>(null)
const discardTrigger = ref<HTMLButtonElement | null>(null)
const deckDraft = ref<PlayerDeckList>({ name: '', cards: [] })
const deckSource = ref<'custom' | 'preconstructed'>('preconstructed')
const deckBusy = ref(false)
const deckError = ref('')
const deckImportOpen = ref(false)
const deckImportText = ref('')
const deckImportError = ref('')
const deckImportTrigger = ref<HTMLButtonElement | null>(null)
const deckImportInput = ref<HTMLTextAreaElement | null>(null)
const deckExportStatus = ref('')
const deckExportFailed = ref(false)
let actionDetailTimer: ReturnType<typeof setTimeout> | undefined
let setupRevealTimer: ReturnType<typeof setTimeout> | undefined

const modes = [
  { id: 'duel', icon: '雙', label: '雙人對戰', description: '1 對 1 經典規則' },
  { id: 'team', icon: '隊', label: '團隊對戰', description: '2 對 2 交錯行動' },
]
const ruleGroupLabels = {
  optional: '選用規則',
  advanced: '進階規則',
  theme: '主題規則',
}
const ruleGroups = computed(() => (['optional', 'advanced', 'theme'] as const).map(id => ({
  id,
  label: ruleGroupLabels[id],
  rules: ruleModulePolicy.value.modules
    .map(module => ({
      id: module.id,
      ...presentationForRuleModule(module.id),
      group: module.category,
    }))
    .filter(module => module.group === id),
})))
const ruleLabelById = computed(() => new Map<string, string>(
  ruleGroups.value.flatMap(group => group.rules).map(rule => [rule.id, rule.label]),
))
const deckRules = computed(() => rulesCatalog.value
  ? createDeckCompositionPolicy(rulesCatalog.value.deckComposition)
  : null)
const DECK_ELEMENTS = computed(() => deckRules.value?.elements ?? [])
const DECK_LEVELS = computed(() => deckRules.value?.levels ?? [])
const deckValidation = computed(() => deckRules.value?.validate(deckDraft.value) ?? ({
  valid: false,
  cardCount: deckDraft.value.cards.length,
  levelTotal: 0,
  errors: [],
}))

const playerInitial = computed(() => displayName.value.trim().charAt(0).toUpperCase() || 'A')
const roomModeLabel = computed(() => modes.find((mode) => mode.id === roomMode.value)?.label ?? '')
const onlineMetadata = computed(() => game.metadata.value)
const waitingRoomSummary = computed(() => game.invitation.value
  ? `房號 ${game.invitation.value.roomCode} · ${waitingStatusText.value}`
  : waitingStatusText.value)
const roomWaiting = computed(() => onlineMetadata.value?.status === 'Waiting')
const gameFinished = computed(() => state.value.status === 'Finished')
const firstPlayer = computed<PlayerId | null>(() => (
  roomWaiting.value ? null : state.value.turnOrder[0] ?? null
))
const firstTurnText = computed(() => firstPlayer.value ? `${playerLabel(firstPlayer.value)} 先手` : '尚未決定先手')
const onlinePlayers = computed(() => onlineMetadata.value?.players ?? [])
const waitingStatusText = computed(() => `${onlineMetadata.value?.members.length ?? 0} / ${onlineMetadata.value?.players.length ?? 0} 玩家`)
const currentMember = computed(() => onlineMetadata.value?.members.find(
  (member) => member.userId === currentUserId.value,
))
const ownPlayer = computed(() => currentMember.value?.player ?? '')
const isRoomOwner = computed(() => currentMember.value?.owner === true)
const roomConnected = computed(() => game.connectionState.value === 'connected')
const enabledRuleLabels = computed(() => [
  '基礎規則',
  ...(onlineMetadata.value?.enabledRuleModules ?? [])
    .flatMap(moduleId => {
      const label = ruleLabelById.value.get(moduleId)
      return label ? [label] : []
    }),
])
const visibleEvents = computed<PublicGameEvent[]>(() => roomWaiting.value
  ? game.publicEvents.value
  : [...game.publicEvents.value, {
      id: 'enabled-rules',
      eventType: 'EnabledRules',
      title: '啟用規則',
      summary: enabledRuleLabels.value.join(' · '),
    }])
const canStartOnlineRoom = computed(() => {
  const metadata = onlineMetadata.value

  return Boolean(
    metadata
    && roomConnected.value
    && metadata.members.length === metadata.capacity
    && metadata.members.every((member) => member.connected)
    && metadata.members.every((member) => member.owner || member.ready),
  )
})
type SeatPosition = 'top' | 'right' | 'bottom' | 'left'

interface PlayerSeat {
  player: PlayerId
  position: SeatPosition
}

const playerSeats = computed<PlayerSeat[]>(() => {
  const order = state.value.turnOrder
  const ownIndex = order.indexOf(ownPlayer.value)
  const startIndex = ownIndex >= 0 ? ownIndex : 0
  const relativeOrder = [...order.slice(startIndex), ...order.slice(0, startIndex)]
  const positions: SeatPosition[] = relativeOrder.length === 4
    ? ['bottom', 'left', 'top', 'right']
    : ['bottom', 'top']

  return relativeOrder.map((player, index) => ({
    player,
    position: positions[index] ?? 'top',
  }))
})
const replaySeats = computed<PlayerSeat[]>(() => {
  const order = replayFrame.value?.state.turnOrder ?? []
  const firstPlayer = replayFrame.value?.firstPlayer
  const firstIndex = firstPlayer ? order.indexOf(firstPlayer) : 0
  const relativeOrder = firstIndex > 0
    ? [...order.slice(firstIndex), ...order.slice(0, firstIndex)]
    : order
  const positions: SeatPosition[] = relativeOrder.length === 4
    ? ['bottom', 'left', 'top', 'right']
    : ['bottom', 'top']
  return relativeOrder.map((player, index) => ({
    player,
    position: positions[index] ?? 'top',
  }))
})

function replayPlayerLabel(player: PlayerId): string {
  return replayFrame.value?.players.find(entry => entry.player === player)?.displayName ?? player
}

function replayTeamHp(player: PlayerId): number {
  const state = replayFrame.value?.state
  const team = state?.players.find(entry => entry.id === player)?.team
  return state?.hp.find(entry => entry.team === team)?.hp ?? 0
}

function replayCardsFor(player: PlayerId): CardToken[] {
  const hand = replayFrame.value?.state.hands.find(entry => entry.player === player)
  return cardTokensForRefs(hand?.cards, `replay-hand-${player}`)
}

function replayDeckCount(player: PlayerId): number {
  const cards = replayFrame.value?.state.playerDecks.find(entry => entry.player === player)?.cards
  if (!cards) return 0
  return cards.kind === 'hidden' ? cards.count : cards.cards.length
}

function replayPouch(player: PlayerId): PublicCard | undefined {
  return replayFrame.value?.state.pouches.find(entry => entry.owner === player)?.card ?? undefined
}

function replayCoveredCards(player: PlayerId): PublicCard[] {
  const cards = replayFrame.value?.state.coveredPassives
    .filter(passive => passive.owner === player)
    .flatMap(passive => passive.cards.kind === 'known' ? passive.cards.cards : [])
  return cards ?? []
}

function replayPlayerDiscardCards(player: PlayerId): PublicCard[] {
  return replayFrame.value?.state.playerDiscards.find(entry => entry.player === player)?.cards ?? []
}
const previousFormationCards = computed(() => (
  cardTokensForRefs(state.value.previousTurnFormation?.cards, 'previous-formation')
))
const activeDiscardCards = computed(() => {
  if (!state.value.playerDiscards.length) return state.value.discard
  const owner = activeDiscardOwner.value
    ?? (ownPlayer.value || undefined)
    ?? state.value.currentPlayer
    ?? state.value.playerDiscards[0]?.player
  return state.value.playerDiscards.find(pile => pile.player === owner)?.cards ?? []
})
const visibleDiscardPiles = computed(() => (
  state.value.playerDiscards.length
    ? state.value.playerDiscards.map(pile => ({
        owner: pile.player,
        cards: pile.cards,
        position: playerSeats.value.find(seat => seat.player === pile.player)?.position ?? 'top',
      }))
    : [{ owner: null, cards: state.value.discard, position: null }]
))
const discardComposition = computed(() => buildCardComposition(activeDiscardCards.value))
const activeTeams = computed(() => [...new Set(state.value.players.map((player) => player.team))])
const showSkip = computed(() => (
  roomConnected.value
  && game.interaction.value.canPass
  && game.interaction.value.hasOptionalEffect
))
const visibleNotifications = computed(() => notifications.notifications.value.filter(
  (notification) => screen.value !== 'game' || notification.gameId !== roomCode.value,
))
const appConnectionText = computed(() => {
  if (screen.value === 'game' && !roomRouteError.value && game.onlineGameId.value) {
    return roomConnected.value ? '已連線' : '重新連線中'
  }

  return notifications.connectionState.value === 'connected' ? '已連線' : '連線中'
})
const gameResultText = computed(() => {
  if (state.value.fiveStarAlignment) {
    return `五星連珠 · ${teamLabel(state.value.fiveStarAlignment.team)} 勝利`
  }

  const aliveTeams = state.value.hp.filter((entry) => entry.hp > 0)

  if (aliveTeams.length === 1) {
    return `${teamLabel(aliveTeams[0]!.team)} 勝利`
  }

  return '戰局結束'
})
interface CardToken {
  id: string
  cardId: number
  label: string
  element?: Element | null
  level?: number | null
  hidden: boolean
  selectable: boolean
  selected: boolean
}

async function login() {
  if (!emailInput.value || !passwordInput.value) {
    loginError.value = '請輸入 Email 與密碼'
    return
  }

  if (authMode.value === 'sign-up' && !nameInput.value) {
    loginError.value = '請輸入玩家名稱'
    return
  }

  authBusy.value = true
  loginError.value = ''

  try {
    const result = authMode.value === 'sign-in'
      ? await authClient.signIn.email({
          email: emailInput.value,
          password: passwordInput.value,
          rememberMe: true,
        })
      : await authClient.signUp.email({
          name: nameInput.value,
          email: emailInput.value,
          password: passwordInput.value,
        })

    if (result.error) {
      loginError.value = result.error.message || '無法完成登入'
      return
    }

    const session = await authClient.getSession()
    currentUserId.value = session.data?.user.id ?? ''
    displayName.value = session.data?.user.name || nameInput.value || '玩家'
    notifications.connect()
    roomName.value ||= `${displayName.value}的房間`
    await router.replace(loginRedirect())
  } catch {
    loginError.value = '帳號服務目前無法使用'
  } finally {
    authBusy.value = false
  }
}

async function refreshLocalPasswordResetAvailability() {
  try {
    const response = await $fetch<{ enabled: boolean }>('/api/local-password-reset')
    localPasswordResetEnabled.value = response.enabled === true
  } catch {
    localPasswordResetEnabled.value = false
  }

  return localPasswordResetEnabled.value
}

function openLocalPasswordReset() {
  const redirect = safeInternalPath(route.query.redirect)
  void router.push({
    path: '/reset-password',
    query: redirect ? { redirect } : {},
  })
}

function returnToLogin() {
  const redirect = safeInternalPath(route.query.redirect)
  resetPasswordError.value = ''
  void router.push({
    path: '/login',
    query: redirect ? { redirect } : {},
  })
}

async function resetLocalPassword() {
  if (!resetPasswordEmail.value || !resetPasswordInput.value || !resetPasswordConfirmation.value) {
    resetPasswordError.value = '請輸入 Email、新密碼與確認密碼'
    return
  }

  if (resetPasswordInput.value !== resetPasswordConfirmation.value) {
    resetPasswordError.value = '兩次輸入的新密碼不一致'
    return
  }

  if (resetPasswordInput.value.length < 10) {
    resetPasswordError.value = '密碼至少需要 10 個字元'
    return
  }

  resetPasswordBusy.value = true
  resetPasswordError.value = ''

  try {
    const response = await $fetch<{ status: LocalPasswordResetResult }>('/api/local-password-reset', {
      method: 'POST',
      body: {
        email: resetPasswordEmail.value,
        newPassword: resetPasswordInput.value,
        confirmPassword: resetPasswordConfirmation.value,
      },
    })

    if (response.status === 'user-not-found') {
      resetPasswordError.value = '找不到這個 Email 的帳號'
      return
    }

    if (response.status === 'no-credential') {
      resetPasswordError.value = '此帳號沒有可重設的密碼登入憑證，且不會建立新憑證'
      return
    }

    const result = await authClient.signIn.email({
      email: resetPasswordEmail.value,
      password: resetPasswordInput.value,
      rememberMe: true,
    })

    if (result.error) {
      resetPasswordError.value = result.error.message || '密碼已重設，但自動登入失敗'
      return
    }

    const session = await authClient.getSession()
    currentUserId.value = session.data?.user.id ?? ''
    displayName.value = session.data?.user.name || '玩家'
    notifications.connect()
    roomName.value ||= `${displayName.value}的房間`
    await router.replace(loginRedirect())
  } catch (error) {
    const message = (error as { data?: { statusMessage?: unknown } })?.data?.statusMessage
    resetPasswordError.value = typeof message === 'string' ? message : '本機重設目前無法使用'
  } finally {
    resetPasswordBusy.value = false
  }
}

async function guestLogin() {
  authBusy.value = true
  loginError.value = ''

  try {
    const result = await authClient.signIn.anonymous()

    if (result.error) {
      loginError.value = result.error.message || '無法建立訪客身份'
      return
    }

    const session = await authClient.getSession()
    currentUserId.value = session.data?.user.id ?? ''
    displayName.value = session.data?.user.name || '旅人'
    notifications.connect()
    roomName.value ||= `${displayName.value}的房間`
    await router.replace(loginRedirect())
  } catch {
    loginError.value = '帳號服務目前無法使用'
  } finally {
    authBusy.value = false
  }
}

async function logout() {
  await authClient.signOut()
  notifications.disconnect()
  game.clearRoom()
  profileOpen.value = false
  currentUserId.value = ''
  nameInput.value = ''
  emailInput.value = ''
  passwordInput.value = ''
  roomName.value = ''
  await router.replace('/login')
}

function openDeckEditor() {
  profileOpen.value = false
  void router.push('/deck')
}

function deckElementLabel(element: Element) {
  return deckRules.value?.definitions.find(definition => definition.element === element)?.name
    ?? element
}

function deckCardCount(
  element: Element,
  level: number,
) {
  const id = deckRules.value?.definition(element, level)?.id
  if (!id) return 0
  return deckDraft.value.cards.filter(card => card === id).length
}

function deckCardLimit(element: Element, level: number) {
  return deckRules.value?.definition(element, level)?.personalDeckCopyLimit ?? 0
}

function adjustDeckCard(
  element: Element,
  level: number,
  delta: number,
) {
  const id = deckRules.value?.definition(element, level)?.id
  if (!id) return
  if (delta > 0) {
    deckDraft.value.cards.push(id)
    return
  }
  const index = deckDraft.value.cards.indexOf(id)
  if (index >= 0) deckDraft.value.cards.splice(index, 1)
}

async function openDeckImport() {
  deckImportText.value = ''
  deckImportError.value = ''
  deckImportOpen.value = true
  await nextTick()
  deckImportInput.value?.focus()
}

function closeDeckImport() {
  deckImportOpen.value = false
  deckImportError.value = ''
  void nextTick(() => deckImportTrigger.value?.focus())
}

function trapDeckImportFocus(event: KeyboardEvent) {
  const dialog = event.currentTarget as HTMLElement
  const focusable = [...dialog.querySelectorAll<HTMLElement>(
    'button:not(:disabled), textarea:not(:disabled)',
  )]
  const first = focusable[0]
  const last = focusable.at(-1)
  if (!first || !last) return

  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault()
    last.focus()
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault()
    first.focus()
  }
}

function applyDeckImport() {
  const imported = parsePlayerDeckCounts(deckImportText.value)
  if (!imported.counts) {
    deckImportError.value = imported.error
    return
  }
  if (!deckRules.value) {
    deckImportError.value = '牌組規則尚未載入，請稍後再試。'
    return
  }

  deckDraft.value = deckRules.value.deckFromCounts(deckDraft.value.name, imported.counts)
  deckExportStatus.value = ''
  deckExportFailed.value = false
  closeDeckImport()
}

async function exportDeck() {
  if (!deckRules.value) {
    deckExportFailed.value = true
    deckExportStatus.value = '牌組規則尚未載入，無法匯出。'
    return
  }

  try {
    await navigator.clipboard.writeText(serializePlayerDeckCounts(deckRules.value.counts(deckDraft.value)))
    deckExportFailed.value = false
    deckExportStatus.value = '牌組已複製到剪貼簿。'
  } catch {
    deckExportFailed.value = true
    deckExportStatus.value = '無法寫入剪貼簿，請確認瀏覽器權限後再試。'
  }
}

async function loadDeck() {
  deckBusy.value = true
  deckError.value = ''
  try {
    const response = await $fetch<{
      deck: PlayerDeckList
      source: 'custom' | 'preconstructed'
    }>('/api/deck')
    deckDraft.value = {
      name: response.deck.name,
      cards: [...response.deck.cards],
    }
    deckSource.value = response.source
  } catch (error) {
    deckError.value = presentApiError(error, '無法載入牌組')
  } finally {
    deckBusy.value = false
  }
}

async function saveDeck() {
  if (!deckValidation.value.valid) return
  deckBusy.value = true
  deckError.value = ''
  try {
    const response = await $fetch<{ deck: PlayerDeckList }>('/api/deck', {
      method: 'PUT',
      body: deckDraft.value,
    })
    deckDraft.value = {
      name: response.deck.name,
      cards: [...response.deck.cards],
    }
    deckSource.value = 'custom'
  } catch (error) {
    deckError.value = presentApiError(error, '無法儲存牌組')
  } finally {
    deckBusy.value = false
  }
}

async function resetDeck() {
  deckBusy.value = true
  deckError.value = ''
  try {
    const response = await $fetch<{ deck: PlayerDeckList }>('/api/deck', {
      method: 'DELETE',
    })
    deckDraft.value = {
      name: response.deck.name,
      cards: [...response.deck.cards],
    }
    deckSource.value = 'preconstructed'
  } catch (error) {
    deckError.value = presentApiError(error, '無法重設牌組')
  } finally {
    deckBusy.value = false
  }
}

function toggleAuthMode() {
  authMode.value = authMode.value === 'sign-in' ? 'sign-up' : 'sign-in'
  loginError.value = ''
}

function goHome() {
  if (screen.value === 'game') {
    leaveGame()
    return
  }

  void router.push('/rooms')
}

async function createRoom() {
  activeRoomName.value = roomName.value || `${displayName.value}的房間`
  await createOnlineRoom()
}

async function openRoomSettings() {
  lobbyError.value = ''
  roomSettingsOpen.value = true
  await nextTick()
  roomNameInput.value?.focus()
}

function closeRoomSettings() {
  if (lobbyBusy.value) {
    return
  }

  lobbyError.value = ''
  roomSettingsOpen.value = false
  void nextTick(() => createRoomTrigger.value?.focus())
}

function leaveGame() {
  game.clearRoom()
  void router.push('/rooms')
}

function discardPileUnavailable(cards: readonly unknown[]): boolean {
  return cards.length === 0 || Boolean(state.value.pendingChoice)
}

function toggleDiscardComposition(owner: PlayerId | null, event: MouseEvent) {
  if (discardOpen.value && activeDiscardOwner.value === owner) {
    closeDiscardComposition()
    return
  }

  const pile = visibleDiscardPiles.value.find(candidate => candidate.owner === owner)
  if (!pile || discardPileUnavailable(pile.cards)) {
    return
  }

  discardTrigger.value = event.currentTarget as HTMLButtonElement
  activeDiscardOwner.value = owner
  discardOpen.value = true
}

function closeDiscardComposition() {
  if (!discardOpen.value) {
    return
  }

  discardOpen.value = false
  void nextTick(() => discardTrigger.value?.focus())
}

function closeSplendorMenu(returnFocus = false) {
  if (!splendorMenuOpen.value) {
    return
  }

  splendorMenuOpen.value = false
  if (returnFocus) {
    void nextTick(() => splendorMenuTrigger.value?.focus())
  }
}

function startSplendorAction(ability: PlayableAction) {
  closeSplendorMenu(true)
  void game.performPlayableAction(ability)
}

function handlePageClick(event: MouseEvent) {
  if (
    profileOpen.value
    && event.target instanceof Node
    && !profileMenuContainer.value?.contains(event.target)
  ) {
    profileOpen.value = false
  }
  closeDiscardComposition()
  closeSplendorMenu()
}

function targetsEditableControl(target: EventTarget | null) {
  return target instanceof HTMLInputElement
    || target instanceof HTMLTextAreaElement
    || target instanceof HTMLSelectElement
    || (target instanceof HTMLElement && target.isContentEditable)
}

function handlePageKeydown(event: KeyboardEvent) {
  if (
    event.metaKey
    && !event.ctrlKey
    && !event.altKey
    && !event.shiftKey
    && !event.repeat
    && event.key.toLowerCase() === 'k'
    && !targetsEditableControl(event.target)
    && screen.value === 'game'
    && onlineMetadata.value?.status === 'Active'
    && state.value.status === 'InProgress'
    && viewer.value === state.value.currentPlayer
    && !state.value.pendingChoice
    && !game.isLoading.value
  ) {
    event.preventDefault()
    void game.advanceAutomatic()
    return
  }

  if (event.key !== 'Escape') {
    return
  }

  if (splendorMenuOpen.value) {
    event.preventDefault()
    closeSplendorMenu(true)
    return
  }

  if (discardOpen.value) {
    event.preventDefault()
    closeDiscardComposition()
    return
  }

  if (roomSettingsOpen.value) {
    event.preventDefault()
    closeRoomSettings()
  }
}

async function restartGame() {
  if (await game.resetOnlineRoom()) {
    await refreshRoomLists()
  }
}

async function toggleWaitingRule(moduleId: string) {
  const current = onlineMetadata.value?.enabledRuleModules ?? []
  const next = current.includes(moduleId)
    ? ruleModulePolicy.value.disable(current, moduleId)
    : ruleModulePolicy.value.enable(current, moduleId)
  await game.updateRuleModules(next)
}

async function createOnlineRoom() {
  lobbyBusy.value = true
  lobbyError.value = ''

  try {
    const response = await $fetch<GameRoomResponse>('/api/games', {
      method: 'POST',
      body: {
        name: activeRoomName.value,
        access: roomAccess.value,
        capacity: roomCapacity.value,
      },
    })

    enterOnlineRoom(response)
    await router.push(`/rooms/${encodeURIComponent(response.gameId)}`)
  } catch (error) {
    lobbyError.value = presentApiError(error, '無法建立房間')
  } finally {
    lobbyBusy.value = false
  }
}

async function refreshRoomLists() {
  if (screen.value === 'login') {
    return
  }

  lobbyError.value = ''

  try {
    const response = await $fetch<{
      rooms: PublicRoomSummary[]
      myRooms: PublicRoomSummary[]
    }>('/api/games')
    publicRooms.value = response.rooms
    myRooms.value = response.myRooms
  } catch (error) {
    lobbyError.value = presentApiError(error, '無法取得公開房間')
  }
}

async function joinRoomByCode() {
  if (!joinRoomCode.value) {
    return
  }

  lobbyBusy.value = true
  lobbyError.value = ''

  try {
    const response = await $fetch<GameRoomResponse>('/api/games/join', {
      method: 'POST',
      body: {
        code: joinRoomCode.value.toUpperCase(),
      },
    })

    enterOnlineRoom(response)
    await router.push(`/rooms/${encodeURIComponent(response.gameId)}`)
  } catch (error) {
    lobbyError.value = presentApiError(error, '無法加入房間')
  } finally {
    lobbyBusy.value = false
  }
}

async function startOnlineRoom() {
  if (!game.onlineGameId.value) {
    return
  }

  await game.startOnlineGame()
  await refreshRoomLists()
}

function enterOnlineRoom(response: GameRoomResponse) {
  const ownMember = response.metadata.members.find((member) => member.userId === currentUserId.value)
  viewer.value = ownMember?.player ?? 'observer'
  activeRoomName.value = response.metadata.name
  roomCode.value = response.gameId
  joinRoomCode.value = response.invitation?.roomCode ?? ''
  eventExpanded.value = false
  notifications.dismissRoom(response.gameId)
  game.applyRoomResponse(response)
}

async function openJoinedRoom(gameId: string) {
  await router.push(`/rooms/${encodeURIComponent(gameId)}`)
}

async function enterListedRoom(room: PublicRoomSummary) {
  await openJoinedRoom(room.gameId)
}

async function openNotification(gameId: string, notificationId: string) {
  notifications.dismiss(notificationId)
  await openJoinedRoom(gameId)
}

async function leaveWaitingRoom() {
  if (await game.leaveOnlineRoom()) {
    await router.replace('/rooms')
  }
}

async function removeWaitingPlayer(userId: string) {
  if (await game.removeOnlinePlayer(userId)) {
    await refreshRoomLists()
  }
}

async function dissolveWaitingRoom() {
  if (await game.dissolveOnlineRoom()) {
    await router.replace('/rooms')
  }
}

async function copyInviteLink() {
  const invitation = game.invitation.value
  if (!invitation) {
    return
  }

  const url = new URL(`/rooms/${encodeURIComponent(roomCode.value)}`, window.location.origin)
  url.searchParams.set('invite', invitation.inviteToken)
  await navigator.clipboard.writeText(url.toString())
}

function openReplays() {
  profileOpen.value = false
  void router.push('/replays')
}

function openReplay(replayId: string) {
  void router.push(`/replays/${encodeURIComponent(replayId)}`)
}

async function copyReplayLink(replayId: string) {
  if (!replayId) return
  await navigator.clipboard.writeText(new URL(`/replays/${encodeURIComponent(replayId)}`, window.location.origin).toString())
}

async function loadReplays() {
  replayLoading.value = true
  replayError.value = ''
  try {
    replays.value = (await $fetch<{ replays: ReplaySummary[] }>('/api/replays')).replays
    replayReplaceSource.value = typeof route.query.save === 'string' ? route.query.save : null
  } catch (error) {
    replayError.value = presentApiError(error, '無法取得重播紀錄')
  } finally {
    replayLoading.value = false
  }
}

async function loadReplayFrame(step = 0) {
  if (!replayRouteId.value) return
  replayLoading.value = true
  replayFrame.value = null
  try {
    replayFrame.value = await $fetch<ReplayFrame>(`/api/replays/${encodeURIComponent(replayRouteId.value)}`, { query: { step } })
  } catch {
    replayFrame.value = null
  } finally {
    replayLoading.value = false
  }
}

async function saveCurrentReplay() {
  const sourceGameId = game.savableReplay.value?.sourceGameId
  if (!sourceGameId || replaySaved.value) return
  replaySaving.value = true
  try {
    await $fetch('/api/replays', { method: 'POST', body: { sourceGameId } })
    replaySaved.value = true
  } catch (error: unknown) {
    const code = (error as { data?: { data?: { code?: string } } }).data?.data?.code
    if (code === 'replayLibraryFull') {
      await router.push({ path: '/replays', query: { save: sourceGameId } })
    } else {
      replayError.value = presentApiError(error, '無法儲存本局')
    }
  } finally {
    replaySaving.value = false
  }
}

async function replaceReplay(replaceReplayId: string) {
  const sourceGameId = replayReplaceSource.value
  if (!sourceGameId) return
  try {
    await $fetch('/api/replays', { method: 'POST', body: { sourceGameId, replaceReplayId } })
    replayReplaceSource.value = null
    await loadReplays()
  } catch (error) {
    replayError.value = presentApiError(error, '無法替換重播')
  }
}

async function deleteReplay(replayId: string) {
  if (!window.confirm('確定要刪除此收藏嗎？')) return
  await $fetch(`/api/replays/${encodeURIComponent(replayId)}`, { method: 'DELETE' })
  await loadReplays()
}

function loginRedirect(): string {
  return safeInternalPath(route.query.redirect) ?? '/rooms'
}

function returnToLobby() {
  void router.push('/rooms')
}

async function loadRoomRoute(gameId: string) {
  game.clearRoom()
  roomRouteError.value = ''

  let response: GameRoomResponse
  try {
    response = await $fetch<GameRoomResponse>(`/api/games/${gameId}`)
  } catch {
    try {
      response = await $fetch<GameRoomResponse>(`/api/games/${gameId}/join`, {
        method: 'POST',
        body: {
          invite: typeof route.query.invite === 'string' ? route.query.invite : undefined,
        },
      })
    } catch (error) {
      roomRouteError.value = roomRouteResult(error)
      return
    }
  }

  if (response.metadata.status === 'Dissolved') {
    roomRouteError.value = '找不到這個房間'
    return
  }

  enterOnlineRoom(response)

  if (route.query.invite) {
    await router.replace(`/rooms/${encodeURIComponent(response.gameId)}`)
  }
}

let routeLoadSequence = 0

async function restoreCurrentRoute() {
  const sequence = ++routeLoadSequence
  routeReady.value = false
  roomRouteError.value = ''
  const session = await authClient.getSession()

  if (sequence !== routeLoadSequence) return

  if (!session.data?.user) {
    notifications.disconnect()
    game.clearRoom()
    currentUserId.value = ''

    if (route.path === '/reset-password') {
      if (!await refreshLocalPasswordResetAvailability()) {
        await router.replace('/login')
      }
    } else if (route.path === '/login') {
      void refreshLocalPasswordResetAvailability()
    } else {
      const redirect = safeInternalPath(route.fullPath)
      await router.replace({
        path: '/login',
        query: redirect ? { redirect } : {},
      })
    }

    routeReady.value = true
    return
  }

  currentUserId.value = session.data.user.id
  displayName.value = session.data.user.name || '玩家'
  roomName.value ||= `${displayName.value}的房間`
  notifications.connect()

  if (isAuthenticationScreen.value) {
    await router.replace(loginRedirect())
    return
  }

  if (route.path === '/') {
    await router.replace('/rooms')
    return
  }

  if (route.path.startsWith('/rooms/')) {
    const gameId = typeof route.params.gameId === 'string' ? route.params.gameId : ''
    await loadRoomRoute(gameId)
  } else if (route.path === '/deck') {
    game.clearRoom()
    await loadDeck()
  } else if (route.path === '/replays') {
    game.clearRoom()
    replayFrame.value = null
    await loadReplays()
  } else if (route.path.startsWith('/replays/')) {
    game.clearRoom()
    await loadReplayFrame(0)
  } else {
    game.clearRoom()
    await refreshRoomLists()
  }

  if (sequence === routeLoadSequence) {
    routeReady.value = true
  }
}

onMounted(async () => {
  try {
    rulesCatalog.value = await $fetch<RulesCatalog>('/api/rules/catalog')
    deckDraft.value = deckRules.value?.preconstructedDeck() ?? deckDraft.value
  } catch {
    // Plain Nuxt development has no Worker/WASM bridge; UI-only routes still remain usable.
  }
  window.addEventListener('click', handlePageClick)
  window.addEventListener('keydown', handlePageKeydown)

  watch(
    () => [
      route.path,
      route.params.gameId,
      route.params.replayId,
      route.query.redirect,
      route.query.invite,
      route.query.save,
    ],
    () => {
      void restoreCurrentRoute()
    },
    { immediate: true },
  )
})

onBeforeUnmount(() => {
  window.removeEventListener('click', handlePageClick)
  window.removeEventListener('keydown', handlePageKeydown)
  clearTimeout(actionDetailTimer)
  clearTimeout(setupRevealTimer)
})

watch(
  () => notifications.roomListRevision.value,
  () => {
    if (screen.value !== 'login') {
      void refreshRoomLists()
    }
  },
)

watch(
  () => notifications.notifications.value,
  (items) => {
    const removal = items.find((notification) => (
      notification.gameId === roomCode.value
      && (notification.kind === 'removed' || notification.kind === 'dissolved')
    ))

    if (!removal || screen.value !== 'game') {
      return
    }

    notifications.dismiss(removal.id)
    game.clearRoom()
    void router.replace('/rooms')
  },
  { deep: true },
)

watch(
  () => state.value.discard.length
    + state.value.playerDiscards.reduce((total, pile) => total + pile.cards.length, 0),
  (length) => {
    if (length === 0) {
      closeDiscardComposition()
    }
  },
)

watch(
  () => state.value.pendingChoice,
  (choice) => {
    if (choice) {
      closeDiscardComposition()
    }
  },
)

watch(screen, (value) => {
  if (value !== 'game') {
    closeDiscardComposition()
    closeSplendorMenu()
  }

  if (value !== 'lobby') {
    roomSettingsOpen.value = false
  }
})

watch(
  () => onlineMetadata.value?.status,
  (status, previous) => {
    if (status !== 'Active' || previous === 'Active') {
      return
    }

    clearTimeout(setupRevealTimer)
    showSetupReveal.value = true
    setupRevealTimer = setTimeout(() => {
      showSetupReveal.value = false
    }, 1800)
  },
)

watch(
  () => game.roomDissolved.value,
  (dissolved) => {
    if (dissolved && screen.value === 'game') {
      void router.replace('/rooms')
    }
  },
)

function cardsFor(player: PlayerId): CardToken[] {
  const hand = state.value.hands.find((entry) => entry.player === player)
  if (!hand) return []

  if (hand.cards.kind === 'known') {
    return hand.cards.cards.map((card, index) => ({
      id: `${player}-known-${index}-${card.id}`,
      cardId: card.id,
      label: card.label,
      element: card.element,
      level: card.level,
      hidden: false,
      selectable: game.canSelectCard(player),
      selected: game.selectedCards.value.includes(card.id),
    }))
  }

  if (hand.cards.kind === 'partiallyKnown') {
    return hand.cards.cards.map((card, index) => card
      ? {
          id: `${player}-known-${index}-${card.id}`,
          cardId: card.id,
          label: card.label,
          element: card.element,
          level: card.level,
          hidden: false,
          selectable: false,
          selected: false,
        }
      : {
          id: `${player}-hidden-${index}`,
          cardId: -index - 1,
          label: '',
          hidden: true,
          selectable: false,
          selected: false,
        })
  }

  return Array.from({ length: hand.cards.count }, (_, index) => ({
    id: `${player}-hidden-${index}`,
    cardId: -index - 1,
    label: '',
    hidden: true,
    selectable: false,
    selected: false,
  }))
}

function cardTokensForRefs(cards: PublicCardRefs | undefined, prefix: string): CardToken[] {
  if (!cards) return []

  if (cards.kind === 'known') {
    return cards.cards.map((card, index) => ({
      id: `${prefix}-known-${index}-${card.id}`,
      cardId: card.id,
      label: card.label,
      element: card.element,
      level: card.level,
      hidden: false,
      selectable: false,
      selected: false,
    }))
  }

  if (cards.kind === 'partiallyKnown') {
    return cards.cards.map((card, index) => card
      ? {
          id: `${prefix}-known-${index}-${card.id}`,
          cardId: card.id,
          label: card.label,
          element: card.element,
          level: card.level,
          hidden: false,
          selectable: false,
          selected: false,
        }
      : {
          id: `${prefix}-hidden-${index}`,
          cardId: -index - 1,
          label: '',
          hidden: true,
          selectable: false,
          selected: false,
        })
  }

  return Array.from({ length: cards.count }, (_, index) => ({
    id: `${prefix}-hidden-${index}`,
    cardId: -index - 1,
    label: '',
    hidden: true,
    selectable: false,
    selected: false,
  }))
}

function handCount(player: PlayerId): number {
  const hand = state.value.hands.find((entry) => entry.player === player)
  if (!hand) return 0
  return hand.cards.kind === 'hidden' ? hand.cards.count : hand.cards.cards.length
}

function playerDeckCount(player: PlayerId): number {
  const pile = state.value.playerDecks.find(entry => entry.player === player)
  if (!pile) return 0
  return pile.cards.kind === 'hidden' ? pile.cards.count : pile.cards.cards.length
}

function playerDiscardCount(player: PlayerId): number {
  return state.value.playerDiscards.find(entry => entry.player === player)?.cards.length ?? 0
}

function teamStar(team: TeamId) {
  return state.value.teamStars.find(owned => owned.team === team)?.star
}

function starHistoryLabel(player: PlayerId): string {
  const stars = state.value.starHistories.find(history => history.player === player)?.stars ?? []
  if (!stars.length) return ''

  return stars.map(star => cardElementGlyph(star)).join('、')
}

function spiritFor(player: PlayerId) {
  return state.value.spirits.find(owned => owned.player === player)
}

function spiritLabel(spirit: SpiritKind): string {
  return {
    Metal: '金精靈',
    Wood: '木精靈',
    Water: '水精靈',
    Fire: '火精靈',
    Earth: '土精靈',
    Evil: '惡精靈',
    Death: '死精靈',
  }[spirit]
}

function professionFor(player: PlayerId) {
  return state.value.professions.find(profession => profession.player === player)
}

function cardInterpretationsFor(player: PlayerId) {
  return state.value.cardInterpretations.filter(interpretation => interpretation.player === player)
}

function persistentEffectsFor(player: PlayerId) {
  return presentPersistentEffects(state.value, player, teamForPlayer(player))
}

function professionSummaryLabel(player: PlayerId): string {
  const profession = professionFor(player)
  return profession
    ? `職業 ${profession.name}。能力：${profession.abilities.join('；')}`
    : ''
}

function starLabel(star: import('~/types/fewfc').StarKind): string {
  return {
    Metal: '金星‧太白',
    Wood: '木星‧歲星',
    Water: '水星‧辰星',
    Fire: '火星‧熒惑',
    Earth: '土星‧鎮星',
  }[star]
}

function strategyLabel(strategy: SecretStrategy): string {
  return {
    GoldenCicada: '金蟬',
    StealTheBeam: '偷梁',
    MuddyWaters: '混水',
    WatchTheFire: '觀火',
    LureTheTigerAway: '離山',
    ReturnSoul: '還魂',
    SheepStealing: '牽羊',
    DarkCrossing: '暗渡',
    DeceiveHeaven: '瞞天',
    Retreat: '走為',
  }[strategy]
}

function exposedDeckCards(player: PlayerId) {
  const cards = state.value.playerDecks.find(entry => entry.player === player)?.cards
  return cards?.kind === 'partiallyKnown' ? cards.cards.filter(card => card !== null) : []
}

function counterEffectsFor(player: PlayerId) {
  return state.value.counterEffects.filter((counter) => counter.owner === player)
}

function shieldFor(player: PlayerId): number {
  return state.value.shields.find((shield) => shield.player === player)?.value ?? 0
}

function playerConnected(player: PlayerId): boolean {
  if (player === ownPlayer.value) {
    return roomConnected.value
  }

  return memberForPlayer(player)?.connected ?? false
}

function teamHp(team: TeamId): number {
  return state.value.hp.find((entry) => entry.team === team)?.hp ?? 0
}

function teamLabel(team: TeamId): string {
  if (team === 'team-a') return 'A 隊'
  if (team === 'team-b') return 'B 隊'

  const player = state.value.players.find((candidate) => candidate.team === team)
  return player ? playerLabel(player.id) : team
}

function memberForPlayer(player: PlayerId): GameRoomMember | undefined {
  return onlineMetadata.value?.members.find((member) => member.player === player)
}

function playerLabel(value: PlayerId | null): string {
  if (!value) return '準備開始'
  return memberForPlayer(value)?.displayName ?? value
}

function playerInitialFor(player: PlayerId): string {
  return playerLabel(player).trim().charAt(0).toUpperCase() || '玩'
}

function teamForPlayer(player: PlayerId): TeamId {
  return state.value.players.find((candidate) => candidate.id === player)?.team ?? ''
}

function teamMembers(team: TeamId): string[] {
  return state.value.players
    .filter((player) => player.team === team)
    .map((player) => playerLabel(player.id))
}

function waitingMemberStatus(member: GameRoomMember | undefined): string {
  if (!member) return '等待中'
  if (!member.connected) return '未連線'
  if (member.owner) return '房主'
  return member.ready ? '已準備' : '未準備'
}

function roomStatusLabel(room: PublicRoomSummary): string {
  const status = {
    Waiting: '等待中',
    Active: '對局中',
    Finished: '已結束',
    Dissolved: '已解散',
  }[room.status] ?? room.status

  return `${room.members.length} / ${room.capacity} 玩家 · ${status}`
}

function roomRuleSummary(room: PublicRoomSummary): string {
  return presentRoomRuleDifferences(
    rulesCatalog.value?.ruleModules ?? [],
    room.enabledRuleModules,
  )
}

function roomNeedsAttention(room: PublicRoomSummary): boolean {
  return notifications.notifications.value.some((notification) => (
    notification.gameId === room.gameId
    && (notification.kind === 'gameStarted' || notification.kind === 'yourTurn')
  ))
}

function playableActionName(action: PlayableAction): string {
  if (action.type !== 'performFormation') {
    return action.name
  }
  if (action.matchOption) {
    return `${action.name}（${action.matchOption.preview ?? `指定牌 ${action.matchOption.card}`}）`
  }
  if (!action.starSubstitution) {
    return action.name
  }

  const substitution = action.starSubstitution
  const card = state.value.hands
    .flatMap(hand => hand.cards.kind === 'known' ? hand.cards.cards : [])
    .find(candidate => candidate.id === substitution.card)
  const cardLabel = card?.label ?? `牌 ${substitution.card}`

  return `${action.name}（${cardLabel}：${cardElementLabel(substitution.printedElement)}視為${cardElementLabel(substitution.interpretedElement)}）`
}

function cardElementLabel(element: Element): string {
  return `${environmentLabel(element).replace('環境', '')}牌`
}

function playableAbilityKey(
  ability: Extract<PlayableAction, { type: 'activateProfessionAbility' | 'useSpiritSkill' }>,
): string {
  if (ability.type === 'useSpiritSkill') {
    return `spirit:${ability.id}:${ability.selectedCard ?? ''}:${ability.declaredLevel ?? ''}`
  }
  return `profession:${ability.id}:${ability.cards.join('-')}:${ability.targetCard ?? ''}:${ability.declaredElement ?? ''}:${ability.declaredLevel ?? ''}`
}

function knownCardLabel(cardId: number): string {
  const card = state.value.hands
    .flatMap(hand => hand.cards.kind === 'known' ? hand.cards.cards : [])
    .find(candidate => candidate.id === cardId)
  return card?.label ?? `牌 ${cardId}`
}

function playableActionDetail(action: PlayableAction): string {
  return presentPlayableAction(action, knownCardLabel)
}

function showActionDetail(action: PlayableAction) {
  clearTimeout(actionDetailTimer)
  actionDetail.value = { name: action.name, summary: playableActionDetail(action) }
}

function showTextActionDetail(name: string, summary: string) {
  clearTimeout(actionDetailTimer)
  actionDetail.value = { name, summary }
}

function hideActionDetail() {
  clearTimeout(actionDetailTimer)
  actionDetail.value = null
}

function startActionDetail(action: PlayableAction) {
  clearTimeout(actionDetailTimer)
  actionDetailTimer = setTimeout(() => {
    actionDetail.value = { name: action.name, summary: playableActionDetail(action) }
  }, 450)
}

function cancelActionDetail() {
  clearTimeout(actionDetailTimer)
}

function phaseLabel(value: string): string {
  const labels: Record<string, string> = {
    Main: '主要階段',
    MainPhase: '主要階段',
    TurnStart: '回合開始',
    TurnDraw: '回合抽牌',
    TurnDrawDiscardChoice: '回合抽牌',
    TurnEnd: '回合結束',
  }
  return labels[value] ?? value
}

function environmentLabel(value: PublicGameState['environment']): string {
  if (!value) return '無環境'
  return {
    Metal: '金行',
    Wood: '木行',
    Water: '水行',
    Fire: '火行',
    Earth: '土行',
  }[value]
}

function pendingChoiceLabel(choice: PublicGameState['pendingChoice']): string {
  if (!choice) return '等待選擇'
  return presentPendingChoice(choice.presentation)
}

function formationChoiceLabel(formationId: string): string {
  const labels: Record<string, string> = {
    'echo:ringing-metal': '商調‧鳴金',
    'echo:falling-wood': '角調‧落木',
    'echo:flowing-water': '羽調‧流水',
    'echo:war-fire': '徵調‧戰火',
    'echo:split-earth': '宮調‧裂土',
    'echo:pure-fire': '變徵‧淨火',
    'echo:plant-earth': '變宮‧植土',
  }
  return labels[formationId] ?? formationId
}

function elementClass(element: Element | null | undefined): string {
  return cardElementClass(element)
}

function cardElement(element: Element | null | undefined): string {
  return cardElementGlyph(element)
}

function cardLevel(level: number | null | undefined): string {
  return level === null || level === undefined ? '◆' : String(level)
}

</script>

<style>
@reference "./assets/css/main.css";

.app-shell { @apply min-h-screen bg-ink; }
.deck-page { @apply mx-auto min-h-screen max-w-5xl px-6 pt-28 pb-12; }
.deck-editor { @apply grid gap-5; }
.deck-grid { @apply grid grid-cols-6 gap-2 overflow-x-auto; }
.deck-grid > strong { @apply flex min-h-11 items-center justify-center text-sm; }
.deck-count-control { @apply flex min-w-28 items-center justify-between rounded-lg border border-[#c9c2ae] bg-white p-1; }
.deck-count-control button { @apply grid size-9 place-items-center rounded-md bg-[#e8e2d3] font-bold text-[#18201c] disabled:opacity-35; }
.deck-count-control span { @apply min-w-6 text-center font-bold text-[#18201c]; }
.deck-validation { @apply flex flex-wrap gap-5 rounded-lg border border-emerald-700/30 bg-emerald-50 p-4 text-emerald-900; }
.deck-validation.invalid { @apply border-red-700/30 bg-red-50 text-red-900; }
.deck-validation-errors { @apply grid gap-1 text-sm text-red-700; }
.deck-transfer-status { @apply text-sm text-emerald-700; }
.deck-import-dialog { @apply my-auto w-full max-w-[640px] shadow-[0_24px_70px_rgba(0,0,0,.5)]; }
.deck-management-actions { @apply grid grid-cols-3 gap-3 max-[600px]:grid-cols-1; }
.deck-import-examples { @apply grid gap-2 text-xs text-muted; }
.deck-import-examples code { @apply block whitespace-pre-wrap border border-[#39443d] bg-[#111713] p-2 font-mono text-[#ece8dd]; }
.deck-import-text { @apply min-h-36 w-full border border-[#39443d] bg-[#111713] p-3 font-mono text-[#ece8dd] outline-0 focus:border-[#a57d35] focus:shadow-[0_0_0_2px_rgba(165,125,53,.12)]; }
.rule-toggle { @apply flex items-center gap-2 py-2; }
.waiting-rules { @apply my-4 grid gap-2 border-y border-white/15 py-2; }
.waiting-rules legend { @apply w-full; }
.waiting-rule-group { @apply grid grid-cols-1 gap-1 border border-[#354039] bg-[#121915] p-2 sm:grid-cols-2; }
.waiting-rule-group h3 { @apply col-span-full text-left font-serif text-xs text-gold-light; }
.site-header {
  @apply relative z-20 flex min-h-[84px] items-center justify-between border-b border-[#29322d] bg-[rgba(14,19,16,.96)];
  padding: 10px clamp(14px, 4vw, 64px);
}
.screen-login .site-header { @apply absolute w-full border-0 bg-transparent; }
.brand { @apply flex min-w-0 items-center border-0 bg-transparent p-0; }
.brand-banner { @apply block h-auto w-[min(52vw,456px)] max-w-full rounded-md shadow-[0_10px_28px_rgba(0,0,0,.32)]; }
.brand-mark {
  @apply grid size-[38px] rotate-45 place-items-center border border-[#d6af5d] font-serif font-black text-[#e4c47d];
}
.brand-mark::first-letter { transform: rotate(-45deg); }
.brand > span:last-child { @apply grid text-left; }
.brand strong { @apply font-serif tracking-[.12em]; }
.brand small { @apply text-[8px] tracking-[.38em] text-[#8b948e]; }
.header-actions { @apply relative flex items-center gap-[18px]; }
.connection { @apply text-xs text-[#98a39c]; }
.connection i { display: inline-block; width: 6px; height: 6px; border-radius: 50%; background: #62b585; margin-right: 6px; box-shadow: 0 0 8px #62b585; }
.connection.offline i { background: #c7a35d; box-shadow: none; }
.profile-menu-container { @apply relative; }
.profile-button { @apply flex items-center gap-2 border-0 bg-transparent; }
.avatar { @apply grid size-[34px] place-items-center rounded-full bg-[#b48a47] font-extrabold text-[#141813]; }
.profile-menu { @apply absolute right-0 top-12 min-w-30 border border-[#39443d] bg-[#202822] p-1.5; }
.profile-menu button { @apply w-full border-0 bg-transparent px-3 py-2 text-left; }

.login-layout { @apply grid min-h-screen grid-cols-[1.15fr_.85fr] max-[900px]:grid-cols-1; }
.login-hero {
  @apply relative flex min-h-screen flex-col justify-between overflow-hidden max-[900px]:hidden;
  padding: 140px clamp(40px, 8vw, 130px) 34px;
  background: radial-gradient(circle at 68% 58%, rgba(157, 118, 49, .18), transparent 25%), linear-gradient(145deg, #16201a 0%, #0d120f 68%);
}
.login-hero::before {
  content: ""; position: absolute; inset: 0; opacity: .14;
  background-image: linear-gradient(30deg, #768278 1px, transparent 1px), linear-gradient(150deg, #768278 1px, transparent 1px);
  background-size: 72px 126px;
}
.hero-copy, .hero-footer { @apply relative z-1; }
.kicker, .section-kicker { @apply text-[11px] font-bold tracking-[.28em] text-gold; }
.kicker span { display: inline-block; width: 32px; height: 1px; background: #c6a35e; vertical-align: middle; margin-right: 12px; }
.hero-copy h1 { @apply my-[22px] font-serif leading-[1.22] tracking-[.04em]; font-size: clamp(48px, 5.8vw, 88px); }
.hero-copy h1 em { @apply not-italic text-[#d1ad62]; }
.hero-description { @apply max-w-[510px] text-sm leading-[1.95] text-[#a7b0a9]; }
.hero-copy h1, .hero-description { @apply relative z-1; }
.hero-footer { @apply flex justify-between text-[11px] text-[#687169]; }
.element-orbit { position: absolute; width: 460px; height: 460px; border: 1px solid rgba(195, 157, 87, .18); border-radius: 50%; left: 55%; top: 72%; transform: translate(-50%, -50%); }
.element-orbit::after { content: ""; position: absolute; inset: 50px; border: 1px dashed rgba(195, 157, 87, .14); border-radius: 50%; }
.orbit-core { position: absolute; inset: 50%; width: 86px; height: 86px; margin: -43px; border: 1px solid #826b3f; border-radius: 50%; display: grid; place-items: center; color: #d4b46e; font-family: "Noto Serif TC", serif; font-size: 22px; background: #131a16; }
.element { position: absolute; width: 54px; height: 54px; border-radius: 50%; display: grid; place-items: center; font-family: "Noto Serif TC", serif; border: 1px solid currentColor; background: #151c18; }
.element.metal { color: #ded5ba; left: 203px; top: -27px; }
.element.wood { color: #6ba77b; right: 7px; top: 120px; }
.element.water { color: #6197af; right: 58px; bottom: 35px; }
.element.fire { color: #c76655; left: 58px; bottom: 35px; }
.element.earth { color: #bd9656; left: 7px; top: 120px; }

.login-panel { @apply grid place-items-center bg-[#f1eee5] px-[30px] pt-20 pb-[30px] text-[#18201c] max-[900px]:min-h-screen; }
.auth-card { @apply w-full max-w-100; }
.mobile-brand { @apply hidden max-[900px]:mb-15 max-[900px]:flex max-[900px]:items-center max-[900px]:gap-[15px] max-[900px]:font-serif max-[900px]:font-extrabold; }
.auth-card h2, .lobby-heading h1 { @apply my-2.5 mb-2 font-serif text-4xl; }
.muted { @apply text-[13px] leading-[1.7] text-[#707872]; }
.auth-card form { @apply mt-[30px]; }
.auth-card label, .setup-card > label, fieldset legend { @apply mb-[9px] block text-xs font-bold; }
.auth-card form label:not(:first-child) { @apply mt-3.5; }
.input-wrap { @apply flex h-13 items-center gap-3 border border-[#c7c8c0] bg-[#faf9f5] px-4; }
.input-wrap span { @apply font-serif text-[#9a8151]; }
.input-wrap input, .text-input, .code-input { @apply w-full border-0 bg-transparent text-[#18201c] outline-0; }
.input-wrap:focus-within, .text-input:focus { border-color: #a57d35; box-shadow: 0 0 0 2px rgba(165, 125, 53, .12); }
.primary-button {
  @apply flex min-h-[50px] items-center justify-between border border-[#b99550] px-[22px] font-bold text-white hover:brightness-[1.08];
  background: linear-gradient(135deg, #b58c43, #8f6a2d);
}
.login-button { @apply mt-4 w-full; }
.auth-mode-button { @apply mt-[15px] w-full border-0 bg-transparent text-[11px] text-[#82672f]; }
.divider { @apply my-8 h-px bg-[#d2d1ca] text-center; }
.divider span { position: relative; top: -10px; background: #f1eee5; padding: 0 16px; color: #969b96; font-size: 11px; }
.ghost-button { @apply min-h-12 w-full border border-[#bcbdb7] bg-transparent text-[#323a35]; }
.terms { @apply mt-6 text-center text-[10px] text-[#9a9e9a]; }
.form-error { @apply mt-2 text-xs text-[#c84d45]; }

.lobby-page { @apply mx-auto max-w-[1180px] px-[30px] pt-15 pb-[90px] max-[600px]:px-4 max-[600px]:py-9; }
.lobby-heading { @apply mb-[38px] flex items-end justify-between gap-6 max-[900px]:flex-col max-[900px]:items-start; }
.lobby-actions { @apply flex items-stretch gap-3 max-[600px]:w-full max-[600px]:flex-col-reverse; }
.room-code-form { @apply flex min-h-[50px] border border-[#3a443e] bg-[#111713] focus-within:border-[#b99550]; }
.room-code-form .code-input { @apply h-auto min-w-48 border-0 px-4 text-left tracking-[.08em] max-[600px]:min-w-0; }
.room-code-form button { @apply border-0 border-l border-[#3a443e] bg-[#222a25] px-4 text-xs text-gold-light disabled:cursor-not-allowed disabled:opacity-45; }
.create-room-button { @apply min-w-35 justify-center; }
.lobby-error { @apply mb-2 border border-[#6b3532] bg-[#2a1817] p-3; }
.room-settings-layer { @apply fixed inset-0 z-40 grid place-items-center overflow-y-auto bg-[rgba(7,10,8,.76)] p-5 backdrop-blur-[3px]; }
.setup-card { @apply border border-line bg-panel p-8 max-[600px]:px-[18px] max-[600px]:py-[22px]; }
.room-settings-dialog { @apply my-auto w-full max-w-[720px] shadow-[0_24px_70px_rgba(0,0,0,.5)]; }
.card-heading { @apply mb-8 flex gap-[18px]; }
.step-number { @apply grid size-[42px] place-items-center border border-[#7e693e] font-serif text-[#d3ae62]; }
.card-heading h2 { @apply mb-1 font-serif text-[21px]; }
.card-heading p { @apply text-xs text-muted; }
.text-input { @apply mb-[26px] h-12 border border-[#39443d] bg-[#111713] px-3.5 text-[#ece8dd]; }
fieldset { @apply mb-[26px] border-0 p-0; }
.option-grid { @apply grid grid-cols-2 gap-3 max-[600px]:grid-cols-1; }
.mode-option { @apply relative grid min-h-27 grid-cols-[42px_1fr] border border-[#354039] bg-[#121814] p-4 text-left text-[#d5d8d4]; }
.mode-option.selected { border-color: #b99550; background: #1d2118; box-shadow: inset 0 0 0 1px #b99550; }
.mode-icon { @apply row-span-2 grid size-8 place-items-center rounded-full bg-[#2b3027] text-[#d8b569]; }
.mode-option small { @apply text-[#758078]; }
.mode-option i { @apply absolute top-2 right-2 text-[9px] not-italic text-[#8a918c]; }
.segmented { @apply grid grid-cols-2 bg-[#111713] p-1; }
.segmented button { @apply min-h-10 border-0 bg-transparent text-muted; }
.segmented button.active { @apply bg-[#293128] text-[#e1c47f]; }
.setup-summary { @apply mb-5 grid gap-2 border border-[#39443d] bg-[#111713] p-4; }
.setup-summary div { @apply flex items-center justify-between gap-4 max-[600px]:grid; }
.setup-summary span { @apply text-[10px] tracking-[.18em] text-muted; }
.setup-summary strong { @apply text-sm text-gold-light; }
.setup-summary p { @apply text-xs text-muted; }
.start-button { @apply w-full; }
.setup-actions { @apply mt-5 grid grid-cols-[auto_1fr] gap-3; }
.secondary-button { @apply min-h-[50px] border border-[#4a554e] bg-transparent px-5 text-muted hover:border-[#b99550] hover:text-gold-light disabled:cursor-not-allowed disabled:opacity-45; }
.code-input { max-width: 320px; height: 52px; border: 1px solid #3a443e; color: white; padding: 0 20px; text-align: center; letter-spacing: .2em; }
.public-rooms-card { @apply mt-0 border border-line bg-panel p-6; }
.my-rooms-card { @apply mt-6 border-[#4a4536]; }
.public-rooms-card .panel-title { @apply mb-4; }
.public-rooms-card .panel-title button { @apply border border-[#3c463f] bg-[#222a25] px-3 py-1 text-[10px]; }
.public-room-list { @apply grid gap-3; }
.public-room-list button { @apply grid grid-cols-[92px_1fr_auto] items-center gap-3 border border-[#354039] bg-[#111713] p-4 text-left hover:border-[#b99550]; }
.public-room-list button:disabled { @apply cursor-not-allowed opacity-55; }
.public-room-list strong { @apply block text-sm text-[#ece8dd]; }
.public-room-list small { @apply text-xs text-muted; }
.public-room-list i { @apply text-[10px] not-italic text-gold-light; }
.room-code { @apply font-mono text-[10px] text-[#8a948d]; overflow-wrap: anywhere; }

.game-page { @apply flex h-[calc(100vh-84px)] flex-col overflow-hidden max-[900px]:h-auto max-[900px]:overflow-visible; }
.back-button { @apply grid size-9 place-items-center border border-[#4a554e] bg-[rgba(17,23,19,.88)] text-base text-[#ddd7c9] hover:border-[#b99550] hover:text-gold-light; }
.battlefield-back { @apply absolute top-4 left-4 z-20; }
.battle-layout { @apply grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_330px] max-[900px]:grid-cols-1 max-[900px]:overflow-auto; }
.battlefield {
  --card-back-base: #232e28;
  --card-back-stripe: #344039;
  --card-back-border: #85714a;
  position: relative;
  min-width: 0;
  display: grid;
  grid-template-areas:
    "top top top"
    "left center right"
    "bottom bottom bottom";
  grid-template-columns: minmax(108px, .6fr) minmax(300px, 1.8fr) minmax(108px, .6fr);
  grid-template-rows: minmax(150px, .8fr) minmax(230px, 1.15fr) minmax(175px, 1fr);
  gap: 8px 14px;
  padding: 22px 36px;
  overflow: hidden;
  background: radial-gradient(ellipse at center, #273029 0%, #141b17 58%, #0f1512 100%);
}
.battlefield::before { content: ""; position: absolute; inset: 22px; border: 1px solid rgba(175, 143, 79, .18); pointer-events: none; }
.battlefield::after { content: "五 行"; position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%); width: 270px; height: 270px; border: 1px solid rgba(183, 148, 77, .1); border-radius: 50%; display: grid; place-items: center; color: rgba(204, 171, 100, .06); font-family: serif; font-size: 70px; pointer-events: none; }
.player-seat { @apply relative z-1 flex min-h-0 min-w-0 items-center justify-center gap-4; }
.seat-top { grid-area: top; }
.seat-bottom { grid-area: bottom; flex-direction: row-reverse; }
.seat-left { grid-area: left; flex-direction: column; }
.seat-right { grid-area: right; flex-direction: column; }
.player-identity { @apply flex min-w-0 flex-wrap items-center gap-2.5; }
.player-identity div { @apply grid; }
.player-identity strong { @apply max-w-36 truncate text-xs; }
.player-identity small { @apply text-[11px] text-[#d0a450]; }
.profession-badge { @apply relative cursor-help border border-[#79633b] bg-[#18201b] px-1.5 py-0.5 text-[10px] text-gold-light outline-none focus-visible:border-[#d1ad62]; }
.profession-summary { @apply invisible absolute top-[calc(100%+6px)] left-0 z-20 grid w-64 gap-1 border border-[#64583f] bg-[#18201b] p-2.5 text-left opacity-0 shadow-[0_12px_28px_rgba(0,0,0,.4)]; }
.profession-summary b { @apply font-serif text-xs text-gold-light; }
.profession-summary small { @apply whitespace-normal text-[10px]! leading-4 text-muted!; }
.card-interpretation { @apply border border-[#526c7c] bg-[#17232b] px-1.5 py-0.5 text-[9px]! text-[#b9d5e5]!; }
.profession-badge:hover .profession-summary, .profession-badge:focus .profession-summary, .profession-badge:focus-within .profession-summary { @apply visible opacity-100; }
.connection-dot { @apply size-2 shrink-0 rounded-full border border-[#76524b] bg-[#6f3c34]; }
.connection-dot.connected { @apply border-[#477557] bg-[#63a979]; }
.reconnecting-label { @apply text-[9px] text-[#d0aa5e]; }
.turn-badge { @apply border border-[#477557] bg-[#16251b] px-[7px] py-[3px] text-[9px]! whitespace-nowrap text-[#77bd8d]!; }
.counter-badge { @apply border border-[#8a733b] bg-[#292415] px-[7px] py-[3px] text-[9px]! whitespace-nowrap text-[#d5b868]!; }
.shield-badge { @apply border border-[#557684] bg-[#17262c] px-[7px] py-[3px] text-[9px]! whitespace-nowrap text-[#8fc1d5]!; }
.status-badge { @apply border border-[#765557] bg-[#28191b] px-[7px] py-[3px] text-[9px]! whitespace-nowrap text-[#d49a9a]!; }
.side-hand-count { @apply hidden text-[9px] text-muted; }
.hand { @apply flex min-w-0 items-center justify-center gap-2; }
.playing-card {
  width: clamp(62px, 7vw, 92px); aspect-ratio: 5 / 7; border: 1px solid #79715e; border-radius: 5px;
  --card-face: #ded8c8;
  --card-face-light: #f0eadc;
  --card-border: #79715e;
  --card-ink: #18201c;
  border-color: var(--card-border);
  background-color: var(--card-face);
  background-image: linear-gradient(145deg, var(--card-face-light), var(--card-face));
  color: var(--card-ink);
  @apply relative flex flex-col items-center justify-center p-2 transition-[.18s] max-[600px]:w-[58px];
  box-shadow: 0 5px 15px rgba(0,0,0,.35);
}
.playing-card:enabled:hover, .playing-card.selected { transform: translateY(-14px); border-color: #e2bd67; box-shadow: 0 0 0 2px #c9a451, 0 12px 18px rgba(0,0,0,.45); z-index: 5; }
.playing-card.hidden, .formation-card.hidden {
  background: repeating-linear-gradient(45deg, var(--card-back-base), var(--card-back-base) 5px, var(--card-back-stripe) 5px, var(--card-back-stripe) 10px);
  border: 2px solid var(--card-back-border);
}
.playing-card.hidden::after, .formation-card.hidden::after {
  content: "";
  width: 48%;
  aspect-ratio: 1;
  border: 1px solid rgba(198, 163, 94, .58);
  transform: rotate(45deg);
}
.card-level { @apply absolute top-1 left-1/2 -translate-x-1/2 font-serif text-xs leading-none font-extrabold; }
.card-element { @apply grid size-[35px] place-items-center rounded-full border border-current font-serif text-lg; color: var(--card-ink); }
.playing-card.element-Metal { --card-face: #ddd5b5; --card-face-light: #f3eed8; --card-border: #89783e; --card-ink: #67571e; }
.playing-card.element-Wood { --card-face: #cfe0ce; --card-face-light: #e8f1e5; --card-border: #52765a; --card-ink: #315f3d; }
.playing-card.element-Water { --card-face: #cbdfe8; --card-face-light: #e7f1f5; --card-border: #4d7890; --card-ink: #245d78; }
.playing-card.element-Fire { --card-face: #ead0c9; --card-face-light: #f6e7e2; --card-border: #985448; --card-ink: #8d3026; }
.playing-card.element-Earth { --card-face: #e4d5ba; --card-face-light: #f3ead8; --card-border: #936d3b; --card-ink: #785027; }
.seat-top .playing-card { width: clamp(48px, 5vw, 68px); }
.seat-top .card-element { @apply size-7 text-sm; }
.seat-left .seat-hand, .seat-right .seat-hand { @apply flex-col gap-1; }
.seat-left .playing-card, .seat-right .playing-card { width: 30px; }
.seat-left .card-level, .seat-right .card-level { @apply top-0.5 text-[8px]; }
.seat-left .card-element, .seat-right .card-element { @apply size-4 text-[10px]; }
.board-center { grid-area: center; @apply relative z-1 grid min-w-0 grid-cols-[90px_minmax(220px,1fr)_90px] items-center justify-items-center; }
.battlefield.discard-open { z-index: 25; overflow: visible; }
.battlefield.discard-open .board-center { z-index: 16; }
.discard-piles { @apply z-2 flex w-full justify-end; grid-column: 1 / -1; grid-row: 1; }
.discard-piles.personal { @apply pointer-events-none absolute inset-0 block; }
.discard-pile { @apply grid justify-items-center gap-1.5 text-[9px] text-[#707b73]; }
.discard-piles.personal .discard-pile { @apply pointer-events-auto absolute w-[90px]; }
.discard-position-top { top: 0; left: 0; }
.discard-position-left { bottom: 0; left: 0; }
.discard-position-right { top: 0; right: 0; }
.discard-position-bottom { right: 0; bottom: 0; }
.discard-pile-trigger { @apply grid w-[52px] place-items-center border border-[#665b44] bg-[#18201b] font-serif text-xl text-[#a68d56]; aspect-ratio: 5/7; }
.discard-pile-trigger { @apply p-0 hover:border-[#b99550] hover:text-gold-light; }
.discard-pile-trigger:focus-visible { outline: 2px solid #d1ad62; outline-offset: 3px; }
.discard-pile.disabled .discard-pile-trigger { @apply cursor-not-allowed opacity-45; }
.discard-composition-layer { @apply absolute right-0 z-20; bottom: calc(50% + 48px); }
.discard-composition {
  @apply w-[300px] border border-[#8e733d] bg-[#18201b] p-3.5 text-[#ece8dd] shadow-[0_18px_48px_rgba(0,0,0,.52)];
}
.discard-composition h2 { @apply mb-2.5 font-serif text-sm text-gold-light; }
.card-composition table { @apply w-full table-fixed border-collapse; }
.card-composition th, .card-composition td { @apply h-8 border border-[#354039] text-center; }
.card-composition thead th { @apply text-[10px] font-bold text-[#d5d8d4]; }
.card-composition tbody th { @apply w-7 text-[10px] font-normal text-muted; }
.card-composition td strong { @apply font-serif text-sm text-[#e4c47d]; }
.card-composition td.empty strong { @apply text-[#59635c]; }
.card-composition tbody tr:nth-child(1) > th { color: #ded5ba; }
.card-composition tbody tr:nth-child(2) > th { color: #77a980; }
.card-composition tbody tr:nth-child(3) > th { color: #75a8bd; }
.card-composition tbody tr:nth-child(4) > th { color: #d17a6c; }
.card-composition tbody tr:nth-child(5) > th { color: #c8a265; }
.pouch-composition { @apply mx-auto mt-5 w-[min(390px,calc(100vw-64px))] border border-[#8e733d] bg-[#18201b] p-3.5 text-[#ece8dd] shadow-[0_18px_48px_rgba(0,0,0,.52)]; }
.pouch-composition td { @apply p-0; }
.pouch-composition td button { @apply grid size-full min-h-8 place-items-center border-0 bg-transparent text-[#e4c47d] hover:bg-[rgba(185,149,80,.16)] disabled:cursor-not-allowed disabled:opacity-45; }
.pouch-composition td button[aria-pressed="true"] { @apply bg-[rgba(185,149,80,.3)] shadow-[inset_0_0_0_2px_#d1ad62]; }
.choice-card-matrix td button small { @apply text-[8px] font-normal text-[#f0d99e]; }
.chain-composition { @apply mt-2; }
.choice-selection-summary { @apply mx-auto mb-1 flex max-w-[390px] items-center justify-between gap-3 text-xs text-gold-light; }
.choice-selection-summary button { @apply border border-[#665b44] bg-[#18201b] px-2 py-1 text-[10px] text-[#d5d8d4] hover:border-[#b99550]; }
.formation-field { @apply relative z-3 grid min-h-48 w-full min-w-0 grid-rows-[auto_1fr_auto] items-center border-x border-[rgba(166,141,86,.14)] px-3 py-2 text-center text-[10px] text-[#69736c]; grid-column: 2; grid-row: 1; }
.formation-field-heading { @apply flex flex-wrap items-center justify-center gap-2; }
.formation-field-label { @apply text-[#9a8251]; letter-spacing: .2em; }
.environment-badge { @apply border border-[#64583f] bg-[#1a211c] px-2 py-1 text-[9px] text-gold-light; }
.previous-formation { @apply grid min-h-24 content-center justify-items-center gap-1.5; }
.previous-formation small { @apply text-[9px] text-muted; }
.previous-formation strong { @apply font-serif text-sm text-gold-light; }
.previous-formation p { @apply text-[10px] text-[#68726b]; }
.formation-cards { @apply flex min-h-12 items-center justify-center; }
.formation-card { @apply relative grid w-8 place-items-center rounded-[3px] border border-[#79715e] bg-[#d8cfba] text-[#18201c]; aspect-ratio: 5/7; margin-left: -4px; }
.formation-card i { @apply absolute top-0.5 left-1 text-[8px] not-italic; }
.formation-card b { @apply font-serif text-xs; }
.turn-controls { @apply relative grid min-h-14 content-center gap-2 border-t border-[rgba(166,141,86,.14)] pt-2; }
.ability-panel, .action-panel { @apply grid gap-1.5 border border-[rgba(166,141,86,.18)] bg-[rgba(17,23,19,.45)] p-2; }
.ability-panel header, .action-panel header { @apply flex flex-wrap items-baseline justify-between gap-x-2 text-left; }
.ability-panel h3, .action-panel h3 { @apply font-serif text-xs text-gold-light; }
.ability-panel small, .action-panel small { @apply text-[9px] text-muted; }
.action-candidates { @apply flex max-w-full flex-wrap justify-center gap-1.5; }
.action-candidates button { @apply min-h-8 border border-[#4c554f] bg-[#18201b] px-2.5 py-1.5 text-[10px] text-[#e1ddd2] hover:border-[#b99550]; }
.action-candidates p { @apply text-[9px] text-[#68726b]; }
.spirit-level-picker { @apply relative; }
.spirit-level-trigger { @apply grid min-h-8 min-w-12 place-items-center border border-[#4c554f] bg-[#18201b] px-2.5 py-1.5 text-[10px] text-[#e1ddd2] hover:border-[#b99550]; }
.spirit-level-trigger:focus-visible { outline: 2px solid #d1ad62; outline-offset: 2px; }
.spirit-level-options { @apply absolute bottom-[calc(100%+5px)] left-1/2 z-10 grid min-w-20 -translate-x-1/2 gap-1 border border-[#64583f] bg-[#121915] p-1 shadow-[0_10px_24px_rgba(0,0,0,.45)]; }
.action-candidates .spirit-level-options button { @apply min-h-7 whitespace-nowrap px-2 py-1; }
.action-candidates .skip-action { @apply border-[#79633b] text-gold-light; }
.action-processing { @apply text-[#d0aa5e]; }
.action-error { @apply text-[#d79587]; }
.action-prompt { @apply text-[#68726b]; }
.action-detail { @apply absolute right-0 bottom-[calc(100%+8px)] left-0 z-8 border border-[#64583f] bg-[#1c241f] p-3 text-left text-xs leading-5 text-muted shadow-[0_12px_28px_rgba(0,0,0,.4)]; }
.action-detail strong { @apply mr-2 text-gold-light; }
.choice-overlay,
.choice-waiting-overlay { @apply absolute inset-0 z-12 grid place-items-center bg-[rgba(7,10,8,.28)] text-center; }
.choice-overlay > div,
.choice-waiting-overlay > div { @apply max-h-[calc(100%-32px)] min-w-90 overflow-y-auto border border-[#8e733d] bg-[rgba(24,32,27,.94)] p-[30px] shadow-[0_18px_48px_rgba(0,0,0,.42)]; }
.choice-overlay h2,
.choice-waiting-overlay h2 { @apply mt-2.5 mb-5 font-serif; }
.choice-cards { @apply flex max-w-[min(620px,calc(100vw-48px))] flex-wrap justify-center gap-2; }
.choice-cards button { @apply border border-[#ae8b47] bg-[#ede6d4] p-2.5 text-[#18201c]; }
.choice-cards button.selected { @apply bg-[#c9a451] font-bold shadow-[0_0_0_2px_#f0d99e]; }
.choice-options { @apply mt-3 flex max-w-[min(620px,calc(100vw-48px))] flex-wrap justify-center gap-2; }
.choice-options button { @apply min-h-10 border border-[#59635c] bg-[#18201b] px-3 py-2 text-xs text-[#d5d8d4] hover:border-[#b99550] hover:text-gold-light disabled:cursor-not-allowed disabled:opacity-45; }
.choice-options button:focus-visible { @apply border-[#d1ad62] outline-2 outline-offset-2 outline-[#d1ad62]; }
.choice-options button.selected,
.choice-options button[aria-pressed="true"] { @apply border-[#d1ad62] bg-[#c9a451] font-bold text-[#121713] shadow-[0_0_0_2px_#f0d99e]; }
.choice-actions { @apply mt-5; }
.choice-actions .choice-confirm { @apply border-[#b99550] bg-[#b99550] font-bold text-[#121713] hover:bg-[#c9a451] hover:text-[#121713]; }
.choice-count { @apply mt-4 text-xs text-muted; }
.choice-submit { @apply mt-3 border border-[#b99550] bg-[#b99550] px-5 py-2 text-xs font-bold text-[#121713] disabled:cursor-not-allowed disabled:opacity-45; }
.setup-reveal { @apply absolute inset-0 z-15 grid place-items-center bg-[rgba(7,10,8,.88)] text-center backdrop-blur-[5px]; }
.setup-reveal > div { @apply grid w-[min(520px,calc(100vw-32px))] gap-4 border border-[#b99550] bg-[#18201b] p-7; }
.setup-reveal h2 { @apply font-serif text-2xl text-gold-light; }
.setup-reveal ol { @apply m-0 grid list-none grid-cols-4 gap-2 p-0 max-[600px]:grid-cols-2; counter-reset: order; }
.setup-reveal li { @apply border border-[#39443d] bg-[#111713] p-2 text-xs; counter-increment: order; }
.setup-reveal li::before { content: counter(order) ". "; color: #c6a35e; }
.revealed-teams { @apply grid grid-cols-2 gap-3; }
.revealed-teams span { @apply grid gap-1 border border-[#39443d] p-3 text-xs text-muted; }
.revealed-teams strong { @apply text-gold-light; }
.waiting-overlay { @apply absolute inset-0 flex items-start justify-center overflow-y-auto bg-[rgba(7,10,8,.78)] py-4 backdrop-blur-[4px]; z-index: 13; }
.waiting-panel { width: min(860px, calc(100% - 64px)); @apply my-auto grid grid-cols-[minmax(0,1.45fr)_minmax(240px,.75fr)] gap-6 border border-[#8e733d] bg-[#18201b] p-7 text-left shadow-[0_24px_80px_rgba(0,0,0,.42)]; }
.waiting-main { @apply min-w-0; }
.waiting-side { @apply grid min-w-0 content-start gap-4 border-l border-[#354039] pl-6; }
.waiting-overlay h2 { @apply font-serif text-3xl text-gold-light; }
.waiting-overlay p:not(.section-kicker) { @apply text-sm text-muted; }
.waiting-members { @apply grid grid-cols-2 gap-3; }
.waiting-members span { @apply grid gap-1 border border-[#354039] bg-[#111713] p-3 text-sm text-muted; }
.waiting-members span.joined { @apply border-[#b99550] text-[#ece8dd]; }
.waiting-members small { @apply text-[10px] text-muted; }
.waiting-members button { @apply mt-1 border-0 bg-transparent text-[9px] text-[#c98e82]; }
.waiting-side .result-actions { @apply mt-0; }
.invite-link { @apply justify-self-start border-0 bg-transparent text-xs text-gold-light; }
.result-actions { @apply mt-2 grid grid-cols-2 gap-3; }
.result-actions .ghost-button { @apply border-[#59635c] text-[#ece8dd]; }
.result-actions .primary-button { @apply justify-between; }

.game-sidebar { @apply grid min-h-0 grid-rows-[minmax(0,1fr)] overflow-hidden border-l border-line bg-panel max-[900px]:border-l-0; }
.game-sidebar.finished { grid-template-rows: auto minmax(0, 1fr); }
.result-panel { @apply border-b border-[#8e733d] bg-[#18201b] p-5; }
.result-panel h2 { @apply font-serif text-2xl text-gold-light; }
.result-panel p { @apply mt-1 text-xs text-muted; }
.result-panel .result-actions { @apply grid-cols-1; }
.panel-title { @apply flex items-start justify-between; }
.event-panel { @apply min-h-0 overflow-auto border-b border-line p-5; }
.panel-title h2 { @apply font-serif text-[15px]; }
.event-panel .panel-title button { @apply hidden border-0 bg-transparent text-[10px] text-gold-light; }
.event-feed { @apply mt-4 grid list-none gap-[13px] p-0; }
.event-feed li { @apply grid grid-cols-[10px_1fr] gap-[7px]; }
.event-feed li > i { width: 5px; height: 5px; border-radius: 50%; background: #b79550; margin-top: 6px; box-shadow: 0 0 0 4px rgba(183, 149, 80, .08); }
.event-feed span { color: #d4d8d4; font-size: 10px; font-weight: 700; }
.event-feed p { color: #6f7972; font-size: 9px; line-height: 1.45; margin-top: 2px; }
.notification-stack { @apply fixed top-24 right-5 z-30 grid w-[min(360px,calc(100vw-32px))] gap-2; }
.notification-item { @apply grid grid-cols-[1fr_34px] border border-[#8e733d] bg-[#18201b] shadow-[0_12px_36px_rgba(0,0,0,.4)]; }
.notification-main { @apply grid gap-1 border-0 bg-transparent p-3 text-left; }
.notification-main strong { @apply text-xs text-[#ece8dd]; }
.notification-main span { @apply text-[10px] text-gold-light; }
.notification-dismiss { @apply border-0 border-l border-line bg-transparent text-muted; }

@media (min-width: 901px) {
  .player-identity strong { font-size: 14px; }
  .player-identity small { font-size: 12px; }
  .reconnecting-label, .turn-badge, .counter-badge, .shield-badge, .status-badge, .side-hand-count { font-size: 11px!important; }
  .discard-pile { font-size: 11px; }
  .formation-field { font-size: 12px; }
  .previous-formation small { font-size: 11px; }
  .previous-formation p, .action-candidates button { font-size: 12px; }
  .panel-title h2 { font-size: 17px; }
  .event-feed span { font-size: 12px; }
  .event-feed p { font-size: 11px; }
}

@media (max-width: 900px) {
  .login-layout { grid-template-columns: 1fr; }
  .login-hero { display: none; }
  .screen-login .site-header { position: relative; border-bottom: 1px solid #29322d; background: rgba(14, 19, 16, .96); }
  .mobile-brand { display: none; }
  .login-panel { min-height: calc(100vh - 84px); }
  .lobby-heading { align-items: start; gap: 25px; flex-direction: column; }
  .battle-layout { grid-template-columns: 1fr; overflow: auto; }
  .game-page { height: auto; overflow: visible; }
  .battlefield { min-height: 720px; padding: 22px; }
  .discard-composition-layer {
    @apply fixed inset-0 grid place-items-center bg-[rgba(7,10,8,.72)] p-4 backdrop-blur-[3px];
  }
  .discard-composition { width: min(330px, calc(100vw - 32px)); }
  .game-sidebar { border-left: 0; }
  .event-panel .panel-title button { display: block; }
  .event-panel:not(.expanded) .event-feed li:nth-child(n+4) { display: none; }
}

@media (max-width: 600px) {
  .site-header { min-height: 68px; padding: 8px 12px; }
  .brand-banner { width: min(70vw, 300px); }
  .screen-login .login-panel { min-height: calc(100vh - 68px); }
  .connection, .profile-button > span:nth-child(2) { display: none; }
  .lobby-page { padding: 36px 16px; }
  .setup-card { padding: 22px 18px; }
  .option-grid { grid-template-columns: 1fr; }
  .battlefield {
    min-height: 690px;
    grid-template-columns: 76px minmax(0, 1fr) 76px;
    grid-template-rows: 165px minmax(250px, 1fr) 185px;
    gap: 4px;
    padding: 46px 8px 12px;
  }
  .battlefield::before { inset: 8px; }
  .battlefield-back { top: 10px; left: 10px; }
  .player-seat { gap: 7px; }
  .seat-top, .seat-bottom { flex-direction: column; }
  .seat-top .player-identity, .seat-bottom .player-identity { order: 2; }
  .seat-left .seat-hand, .seat-right .seat-hand { display: none; }
  .seat-left .player-identity, .seat-right .player-identity { @apply flex-col gap-1 text-center; }
  .seat-left .player-identity strong, .seat-right .player-identity strong { @apply max-w-18 text-[9px]; }
  .seat-left .player-identity small, .seat-right .player-identity small { @apply text-[9px]; }
  .seat-left .turn-badge, .seat-right .turn-badge, .seat-left .counter-badge, .seat-right .counter-badge, .seat-left .shield-badge, .seat-right .shield-badge, .seat-left .status-badge, .seat-right .status-badge { @apply max-w-18 whitespace-normal px-1 py-0.5 text-[8px]!; }
  .seat-left .side-hand-count, .seat-right .side-hand-count { @apply block; }
  .board-center { grid-template-columns: 48px minmax(0, 1fr) 48px; width: 100%; }
  .formation-field { min-width: 0; width: 100%; }
  .discard-piles.personal .discard-pile { width: 48px; }
  .discard-pile-trigger { width: 38px; }
  .playing-card { width: 54px; }
  .seat-top .playing-card { width: 43px; }
  .player-identity strong { max-width: 110px; }
  .turn-badge { font-size: 8px!important; }
  .action-candidates button { min-height: 30px; padding: 4px 7px; }
  .waiting-panel { min-width: 0; width: calc(100vw - 32px); grid-template-columns: 1fr; gap: 18px; padding: 22px 16px; }
  .waiting-side { @apply border-t border-l-0 pt-4 pl-0; }
  .waiting-members { grid-template-columns: 1fr 1fr; }
  .notification-stack { top: 76px; right: 16px; }
}
</style>

//! 一般棄牌的來源與落點規則。
//!
//! 這個模組只描述 Card Origin 與 Discard Pile 的事實，不能依賴任何選用規則。

use super::{
    CardInstanceId, CardMoveDelta, CardOrigin, CardZone, EngineInvariantError, GameError,
    GameResult, GameState, PlayerId,
};

#[derive(Debug)]
pub(crate) enum DestinationFact<'a> {
    Derived,
    Recorded(&'a CardZone),
}

/// 已驗證、但尚未放入棄牌堆的普通棄牌。
///
/// 它必須被消耗，避免同一張已移出的卡牌被重複放置。
#[derive(Debug)]
pub(crate) struct PreparedDiscard {
    card: CardInstanceId,
    destination: CardZone,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DiscardLocation {
    Shared,
    Player(PlayerId),
}

impl DiscardLocation {
    pub(crate) fn card_zone(&self) -> CardZone {
        match self {
            Self::Shared => CardZone::Discard,
            Self::Player(player) => CardZone::PlayerDiscard(player.clone()),
        }
    }
}

fn invariant(card: CardInstanceId) -> GameError {
    GameError::EngineInvariant(EngineInvariantError::ZoneOwnershipInconsistency { card })
}

/// 依不可變來源得出唯一的一般棄牌目的地。此映射刻意保持私有。
fn destination_for(state: &GameState, card: CardInstanceId) -> GameResult<CardZone> {
    match state.card_origin(card).ok_or_else(|| invariant(card))? {
        CardOrigin::Shared => Ok(CardZone::Discard),
        CardOrigin::Player(owner) => {
            if !state.players.iter().any(|player| &player.id == owner)
                || !state
                    .player_discards
                    .iter()
                    .any(|pile| &pile.player == owner)
            {
                return Err(invariant(card));
            }
            Ok(CardZone::PlayerDiscard(owner.clone()))
        }
    }
}

/// 規劃普通棄牌的記錄移動。
pub(crate) fn move_from(
    state: &GameState,
    card: CardInstanceId,
    from: CardZone,
) -> GameResult<CardMoveDelta> {
    let prepared = prepare(state, card, DestinationFact::Derived)?;
    Ok(CardMoveDelta {
        card,
        from,
        to: prepared.destination,
    })
}

/// 先驗證一般棄牌目的地。重播時必須提供原始事件的目的地，絕不改寫它。
pub(crate) fn prepare(
    state: &GameState,
    card: CardInstanceId,
    fact: DestinationFact<'_>,
) -> GameResult<PreparedDiscard> {
    let destination = destination_for(state, card)?;
    if let DestinationFact::Recorded(recorded) = fact {
        if recorded != &destination {
            return Err(invariant(card));
        }
    }
    Ok(PreparedDiscard { card, destination })
}

impl PreparedDiscard {
    /// 呼叫端必須先從原區域移除卡牌；這裡只執行已驗證的放置。
    pub(crate) fn place_removed(self, state: &mut GameState) {
        match self.destination {
            CardZone::Discard => state.discard.push(self.card),
            CardZone::PlayerDiscard(owner) => {
                state
                    .discard_for_mut(&owner)
                    .expect("已驗證的一般棄牌必須有對應棄牌堆")
                    .push(self.card);
            }
            _ => unreachable!("一般棄牌只能進入棄牌堆"),
        }
        state
            .exposed_foreign_cards
            .retain(|exposed| *exposed != self.card);
    }
}

/// 尋找卡牌目前唯一的棄牌堆位置。卡牌不在任何棄牌堆時回傳 None。
pub(crate) fn locate(
    state: &GameState,
    card: CardInstanceId,
) -> GameResult<Option<DiscardLocation>> {
    let mut location = state
        .discard
        .contains(&card)
        .then_some(DiscardLocation::Shared);
    for pile in &state.player_discards {
        if pile.cards.contains(&card) {
            if location.is_some() {
                return Err(invariant(card));
            }
            location = Some(DiscardLocation::Player(pile.player.clone()));
        }
    }
    Ok(location)
}

use crate::domain::{
    DeckPlacement, EngineInvariantError, GameError, GameEvent, GameResult, GameState,
    PendingRandomness, PendingResolution, RandomnessDeck, RandomnessOperation, ValidationError,
};

/// 牌堆供給的純規劃結果。呼叫端可用同一個結果判斷候選行動是否可執行，並在
/// 實際解析時請求所需的洗棄牌；它不處理篩選或最終選牌。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DeckSupplyPlan {
    Ready,
    NeedsDiscardShuffle {
        operation: RandomnessOperation,
        current_order: Vec<crate::domain::CardInstanceId>,
    },
}

pub(crate) fn plan(
    state: &GameState,
    deck: &RandomnessDeck,
    required_cards: usize,
    placement: DeckPlacement,
) -> GameResult<DeckSupplyPlan> {
    let (available_deck, discard) = match deck {
        RandomnessDeck::Shared => (&state.deck[..], &state.discard[..]),
        RandomnessDeck::Player(player) => (
            state.deck_for(player).ok_or_else(|| {
                GameError::Validation(ValidationError::UnknownPlayer(player.clone()))
            })?,
            state.discard_for(player).ok_or_else(|| {
                GameError::Validation(ValidationError::UnknownPlayer(player.clone()))
            })?,
        ),
    };

    if available_deck.len() >= required_cards {
        return Ok(DeckSupplyPlan::Ready);
    }

    let available = available_deck.len() + discard.len();
    if available < required_cards {
        return Err(GameError::EngineInvariant(
            EngineInvariantError::NotEnoughCards {
                needed: required_cards,
                available,
            },
        ));
    }

    debug_assert!(
        !discard.is_empty(),
        "可供牌總數足夠時，不足的牌必然存在棄牌堆"
    );
    Ok(DeckSupplyPlan::NeedsDiscardShuffle {
        operation: RandomnessOperation::DiscardShuffle {
            pile: deck.clone(),
            placement,
        },
        current_order: discard.to_vec(),
    })
}

/// 將供給規劃轉為唯一的待隨機性事件。牌堆已足夠時不建立事件；不足時，完成
/// 洗牌後同一個 Pending Resolution 會重新進入對應規則流程。
pub(crate) fn request_if_needed(
    state: &GameState,
    deck: &RandomnessDeck,
    required_cards: usize,
    placement: DeckPlacement,
    request_id: String,
    resolution: PendingResolution,
) -> GameResult<Option<GameEvent>> {
    match plan(state, deck, required_cards, placement)? {
        DeckSupplyPlan::Ready => Ok(None),
        DeckSupplyPlan::NeedsDiscardShuffle {
            operation,
            current_order,
        } => Ok(Some(GameEvent::RandomnessRequested {
            request: PendingRandomness {
                request_id,
                operation,
                current_order,
            },
            resolution,
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::{DeckSupplyPlan, plan};
    use crate::domain::{
        CardInstanceId, DeckPlacement, EngineInvariantError, GameError, GameSetup, GameState,
        PlayerId, RandomnessDeck,
    };

    fn card(id: u64) -> CardInstanceId {
        CardInstanceId::new(id)
    }

    fn state_with_piles(deck: Vec<u64>, discard: Vec<u64>) -> GameState {
        let setup = GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30);
        let mut state = GameState::from_setup(&setup);
        state.deck = deck.into_iter().map(card).collect();
        state.discard = discard.into_iter().map(card).collect();
        state
    }

    #[test]
    fn enough_cards_in_deck_are_ready_without_a_shuffle() {
        let state = state_with_piles(vec![1, 2], vec![3]);

        assert_eq!(
            plan(&state, &RandomnessDeck::Shared, 2, DeckPlacement::Bottom).unwrap(),
            DeckSupplyPlan::Ready
        );
    }

    #[test]
    fn partially_full_deck_still_recycles_discard_for_exact_requirement() {
        let state = state_with_piles(vec![1], vec![2]);

        assert!(matches!(
            plan(&state, &RandomnessDeck::Shared, 2, DeckPlacement::Bottom).unwrap(),
            DeckSupplyPlan::NeedsDiscardShuffle { current_order, .. } if current_order == vec![card(2)]
        ));
    }

    #[test]
    fn insufficient_combined_piles_are_an_engine_invariant_error() {
        let state = state_with_piles(vec![], vec![1]);

        assert_eq!(
            plan(&state, &RandomnessDeck::Shared, 2, DeckPlacement::Bottom),
            Err(GameError::EngineInvariant(
                EngineInvariantError::NotEnoughCards {
                    needed: 2,
                    available: 1,
                }
            ))
        );
    }
}

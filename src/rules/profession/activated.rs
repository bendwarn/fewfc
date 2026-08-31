//! 啟用職業能力的共同生命週期。
//!
//! 各 Rule Module 只描述自己能提供的能力及其效果計畫；本模組統一負責能力
//! 所有權、每回合一次、選牌與補齊輸入驗證、呈現資料，以及標準事件排序。

use std::collections::HashSet;

use crate::domain::{
    CardInstanceId, Element, EngineInvariantError, GameError, GameEvent, GameResult, GameState,
    PlayerId, PreparedProfessionAbility, RuleImplementationError, ValidationError,
};
use crate::rules::{
    ActionCost, ActionInputRequirement, ConsequenceCertainty, ImmediateEffect,
    PlayerFacingActionDetail, ProfessionAbilityCandidate, ProfessionAbilityEffect, RuleConsequence,
    RuleException,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActivatedAbilityKind {
    Hero(crate::rules::hero::ActivatedAbility),
    Jianghu(crate::rules::jianghu::ActivatedAbility),
    Confluence(crate::rules::confluence::ActivatedAbility),
    Dark(crate::rules::dark::ActivatedAbility),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CompletionValue<T> {
    Absent,
    Fixed(T),
    RequiredOneOf(Vec<T>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompletionContract {
    target_card: CompletionValue<CardInstanceId>,
    declared_element: CompletionValue<Element>,
    declared_level: CompletionValue<u32>,
}

impl CompletionContract {
    pub(crate) fn absent() -> Self {
        Self {
            target_card: CompletionValue::Absent,
            declared_element: CompletionValue::Absent,
            declared_level: CompletionValue::Absent,
        }
    }

    pub(crate) fn fixed_target(card: CardInstanceId) -> Self {
        Self {
            target_card: CompletionValue::Fixed(card),
            ..Self::absent()
        }
    }

    pub(crate) fn fixed_target_element_and_level(
        card: CardInstanceId,
        element: Element,
        level: u32,
    ) -> Self {
        Self {
            target_card: CompletionValue::Fixed(card),
            declared_element: CompletionValue::Fixed(element),
            declared_level: CompletionValue::Fixed(level),
        }
    }

    pub(crate) fn virtual_formation_card() -> Self {
        Self {
            target_card: CompletionValue::Absent,
            declared_element: CompletionValue::RequiredOneOf(vec![
                Element::Metal,
                Element::Wood,
                Element::Water,
                Element::Fire,
                Element::Earth,
            ]),
            declared_level: CompletionValue::RequiredOneOf((1..=5).collect()),
        }
    }

    fn matches(&self, input: &CompletedAbilityInput) -> bool {
        completion_value_matches(&self.target_card, input.target_card)
            && completion_value_matches(&self.declared_element, input.declared_element)
            && completion_value_matches(&self.declared_level, input.declared_level)
    }

    fn target_card(&self) -> Option<CardInstanceId> {
        fixed_value(&self.target_card)
    }

    fn declared_element(&self) -> Option<Element> {
        fixed_value(&self.declared_element)
    }

    fn declared_level(&self) -> Option<u32> {
        fixed_value(&self.declared_level)
    }

    fn input_requirement(&self) -> Option<ActionInputRequirement> {
        match (&self.declared_element, &self.declared_level) {
            (CompletionValue::RequiredOneOf(elements), CompletionValue::RequiredOneOf(levels)) => {
                Some(ActionInputRequirement::VirtualFormationCard {
                    elements: elements.clone(),
                    levels: levels.clone(),
                })
            }
            _ => None,
        }
    }
}

fn completion_value_matches<T: PartialEq>(
    contract: &CompletionValue<T>,
    actual: Option<T>,
) -> bool {
    match (contract, actual) {
        (CompletionValue::Absent, None) => true,
        (CompletionValue::Fixed(expected), Some(actual)) => expected == &actual,
        (CompletionValue::RequiredOneOf(allowed), Some(actual)) => allowed.contains(&actual),
        _ => false,
    }
}

fn fixed_value<T: Copy>(contract: &CompletionValue<T>) -> Option<T> {
    match contract {
        CompletionValue::Fixed(value) => Some(*value),
        CompletionValue::Absent | CompletionValue::RequiredOneOf(_) => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AbilityPresentation {
    effect: ProfessionAbilityEffect,
    discards_selected_cards: bool,
    limited_use_key: Option<&'static str>,
}

impl AbilityPresentation {
    pub(crate) fn new(effect: ProfessionAbilityEffect) -> Self {
        Self {
            effect,
            discards_selected_cards: false,
            limited_use_key: None,
        }
    }

    pub(crate) fn discards_selected_cards(mut self) -> Self {
        self.discards_selected_cards = true;
        self
    }

    pub(crate) fn limited_use(mut self, key: &'static str) -> Self {
        self.limited_use_key = Some(key);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AbilityOfferPlan<K> {
    pub(crate) kind: K,
    pub(crate) name: &'static str,
    pub(crate) completion: CompletionContract,
    pub(crate) presentation: AbilityPresentation,
}

impl<K> AbilityOfferPlan<K> {
    pub(crate) fn new(
        kind: K,
        name: &'static str,
        completion: CompletionContract,
        presentation: AbilityPresentation,
    ) -> Self {
        Self {
            kind,
            name,
            completion,
            presentation,
        }
    }
}

pub(crate) struct OfferContext<'a> {
    pub(crate) state: &'a GameState,
    pub(crate) player: &'a PlayerId,
    pub(crate) selection: &'a ValidatedAbilitySelection,
    effective_ability_ids: &'a [&'static str],
}

impl OfferContext<'_> {
    pub(crate) fn has<K, P>(&self, kind: K) -> bool
    where
        K: Copy + Eq,
        P: ActivatedAbilityProvider<Kind = K>,
    {
        self.effective_ability_ids.contains(&P::catalog_id(kind))
    }

    pub(crate) fn has_catalog_id(&self, ability_id: &str) -> bool {
        self.effective_ability_ids.contains(&ability_id)
    }
}

pub(crate) struct ResolveContext<'a> {
    pub(crate) state: &'a GameState,
    pub(crate) player: &'a PlayerId,
    pub(crate) selection: &'a ValidatedAbilitySelection,
    pub(crate) input: &'a CompletedAbilityInput,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CompletedAbilityInput {
    pub(crate) target_card: Option<CardInstanceId>,
    pub(crate) declared_element: Option<Element>,
    pub(crate) declared_level: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ValidatedAbilitySelection {
    cards: Vec<CardInstanceId>,
}

impl ValidatedAbilitySelection {
    fn new(state: &GameState, player: &PlayerId, cards: &[CardInstanceId]) -> GameResult<Self> {
        let hand = state
            .hand(player)
            .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
        let mut seen = HashSet::new();
        for card in cards {
            if !seen.insert(*card) {
                return Err(GameError::Validation(
                    ValidationError::DuplicateSubmittedCard(*card),
                ));
            }
            if !hand.contains(card) {
                return Err(GameError::Validation(ValidationError::CardNotInHand(*card)));
            }
        }
        Ok(Self {
            cards: cards.to_vec(),
        })
    }

    pub(crate) fn cards(&self) -> &[CardInstanceId] {
        &self.cards
    }

    pub(crate) fn len(&self) -> usize {
        self.cards.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    pub(crate) fn first(&self) -> Option<CardInstanceId> {
        self.cards.first().copied()
    }

    fn contains(&self, card: CardInstanceId) -> bool {
        self.cards.contains(&card)
    }
}

pub(crate) trait ActivatedAbilityProvider {
    type Kind: Copy + Eq + std::fmt::Debug + 'static;

    const MODULE_ID: &'static str;

    fn kinds() -> &'static [Self::Kind];
    fn parse(id: &str) -> Option<Self::Kind>;
    fn id(kind: Self::Kind) -> &'static str;
    fn catalog_id(kind: Self::Kind) -> &'static str;
    fn offers(context: &OfferContext<'_>) -> GameResult<Vec<AbilityOfferPlan<Self::Kind>>>;
    fn resolve(
        context: &ResolveContext<'_>,
        kind: Self::Kind,
        builder: AbilityPlanBuilder<'_>,
    ) -> GameResult<AbilityEffectPlan>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AbilityContinuation {
    Choice(GameEvent),
    Randomness(GameEvent),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AbilityEffectPlan {
    prepared: Option<PreparedProfessionAbility>,
    consequences: Vec<GameEvent>,
    continuation: Option<AbilityContinuation>,
}

pub(crate) struct AbilityPlanBuilder<'a> {
    source: &'a GameState,
    player: &'a PlayerId,
    ability_id: &'static str,
    selection: &'a ValidatedAbilitySelection,
    projected: GameState,
    prepared: Option<PreparedProfessionAbility>,
    consequences: Vec<GameEvent>,
    continuation: Option<AbilityContinuation>,
    started: bool,
}

impl<'a> AbilityPlanBuilder<'a> {
    fn new(
        state: &'a GameState,
        player: &'a PlayerId,
        ability_id: &'static str,
        selection: &'a ValidatedAbilitySelection,
    ) -> Self {
        Self {
            source: state,
            player,
            ability_id,
            selection,
            projected: state.clone(),
            prepared: None,
            consequences: Vec::new(),
            continuation: None,
            started: false,
        }
    }

    pub(crate) fn set_prepared(&mut self, prepared: PreparedProfessionAbility) -> GameResult<()> {
        if self.started
            || self.prepared.is_some()
            || prepared.player != *self.player
            || prepared.ability_id != self.ability_id
            || prepared.prepared_on_turn != self.source.turn_number
            || prepared.interpretation_revision != self.source.card_interpretation_revision + 1
            || !self.selection.contains(prepared.card)
        {
            return Err(invalid_plan());
        }
        self.prepared = Some(prepared);
        Ok(())
    }

    pub(crate) fn push_consequence(&mut self, event: GameEvent) -> GameResult<()> {
        if self.continuation.is_some()
            || matches!(
                event,
                GameEvent::ProfessionAbilityActivated { .. }
                    | GameEvent::ChoiceRequested { .. }
                    | GameEvent::RandomnessRequested { .. }
            )
        {
            return Err(invalid_plan());
        }
        self.start();
        crate::rules::projection::apply_event(&mut self.projected, &event);
        self.consequences.push(event);
        Ok(())
    }

    pub(crate) fn projected_state(&mut self) -> &GameState {
        self.start();
        &self.projected
    }

    pub(crate) fn set_continuation(&mut self, event: GameEvent) -> GameResult<()> {
        if self.continuation.is_some() {
            return Err(invalid_plan());
        }
        let continuation = match &event {
            GameEvent::ChoiceRequested { choice, resolution }
                if choice.player == *self.player
                    && crate::rules::pending_resolution::waiting_medium(resolution)
                        == crate::rules::pending_resolution::WaitingMedium::Choice =>
            {
                AbilityContinuation::Choice(event.clone())
            }
            GameEvent::RandomnessRequested { resolution, .. }
                if crate::rules::pending_resolution::waiting_medium(resolution)
                    == crate::rules::pending_resolution::WaitingMedium::Randomness =>
            {
                AbilityContinuation::Randomness(event.clone())
            }
            _ => return Err(invalid_plan()),
        };
        self.start();
        crate::rules::projection::apply_event(&mut self.projected, &event);
        self.continuation = Some(continuation);
        Ok(())
    }

    fn start(&mut self) {
        if self.started {
            return;
        }
        crate::rules::projection::apply_event(
            &mut self.projected,
            &GameEvent::ProfessionAbilityActivated {
                player: self.player.clone(),
                ability_id: self.ability_id.to_string(),
                prepared: self.prepared.clone(),
            },
        );
        self.started = true;
    }

    pub(crate) fn finish(mut self) -> AbilityEffectPlan {
        self.start();
        AbilityEffectPlan {
            prepared: self.prepared,
            consequences: self.consequences,
            continuation: self.continuation,
        }
    }
}

pub(crate) fn offers(
    state: &GameState,
    player: &PlayerId,
    cards: &[CardInstanceId],
) -> GameResult<Vec<ProfessionAbilityCandidate>> {
    if crate::rules::pouch::profession_is_suppressed(state, player)
        || ability_was_activated_this_turn(state, player)
    {
        return Ok(Vec::new());
    }
    let selection = ValidatedAbilitySelection::new(state, player, cards)?;
    let Some(profession) = state.profession_for(player) else {
        return Ok(Vec::new());
    };
    let effective_ability_ids =
        super::effective_ability_ids(&state.enabled_rule_modules, profession);
    let context = OfferContext {
        state,
        player,
        selection: &selection,
        effective_ability_ids: &effective_ability_ids,
    };
    let mut candidates = Vec::new();
    collect_offers::<crate::rules::hero::ActivatedProvider>(&context, &mut candidates)?;
    collect_offers::<crate::rules::jianghu::ActivatedProvider>(&context, &mut candidates)?;
    collect_offers::<crate::rules::confluence::ActivatedProvider>(&context, &mut candidates)?;
    collect_offers::<crate::rules::dark::ActivatedProvider>(&context, &mut candidates)?;
    Ok(candidates)
}

fn collect_offers<P>(
    context: &OfferContext<'_>,
    candidates: &mut Vec<ProfessionAbilityCandidate>,
) -> GameResult<()>
where
    P: ActivatedAbilityProvider,
{
    debug_validate_provider::<P>();
    if !context.state.has_rule_module(P::MODULE_ID) {
        return Ok(());
    }
    for offer in P::offers(context)? {
        if !context.has::<_, P>(offer.kind) {
            return Err(invalid_plan());
        }
        let id = P::id(offer.kind);
        let completion = offer.completion;
        candidates.push(ProfessionAbilityCandidate {
            ability_id: id.to_string(),
            ability_name: offer.name.to_string(),
            cards: context.selection.cards().to_vec(),
            target_card: completion.target_card(),
            declared_element: completion.declared_element(),
            declared_level: completion.declared_level(),
            input_requirement: completion.input_requirement(),
            detail: presentation_detail(
                context.state,
                context.player,
                context.selection,
                offer.presentation,
            ),
        });
    }
    Ok(())
}

fn debug_validate_provider<P: ActivatedAbilityProvider>() {
    #[cfg(debug_assertions)]
    for kind in P::kinds() {
        let id = P::id(*kind);
        debug_assert_eq!(P::parse(id), Some(*kind), "Provider ID 必須可 round-trip");
    }
}

pub(crate) fn activate(
    state: &GameState,
    player: &PlayerId,
    ability_id: &str,
    cards: &[CardInstanceId],
    target_card: Option<CardInstanceId>,
    declared_element: Option<Element>,
    declared_level: Option<u32>,
) -> GameResult<Vec<GameEvent>> {
    let kind = parse(ability_id).ok_or_else(|| {
        GameError::Validation(ValidationError::UnknownProfessionAbility(
            ability_id.to_string(),
        ))
    })?;
    ensure_available(state, player, kind, ability_id)?;
    if ability_was_activated_this_turn(state, player) {
        return Err(GameError::Validation(
            ValidationError::ProfessionAbilityAlreadyActivated {
                player: player.clone(),
                turn_number: state.turn_number,
            },
        ));
    }
    let selection = ValidatedAbilitySelection::new(state, player, cards)?;
    let input = CompletedAbilityInput {
        target_card,
        declared_element,
        declared_level,
    };
    let profession = state.profession_for(player).expect("availability checked");
    let effective_ability_ids =
        super::effective_ability_ids(&state.enabled_rule_modules, profession);
    let offer_context = OfferContext {
        state,
        player,
        selection: &selection,
        effective_ability_ids: &effective_ability_ids,
    };
    let resolve_context = ResolveContext {
        state,
        player,
        selection: &selection,
        input: &input,
    };

    match kind {
        ActivatedAbilityKind::Hero(kind) => activate_with::<crate::rules::hero::ActivatedProvider>(
            &offer_context,
            &resolve_context,
            kind,
        ),
        ActivatedAbilityKind::Jianghu(kind) => activate_with::<
            crate::rules::jianghu::ActivatedProvider,
        >(&offer_context, &resolve_context, kind),
        ActivatedAbilityKind::Confluence(kind) => activate_with::<
            crate::rules::confluence::ActivatedProvider,
        >(&offer_context, &resolve_context, kind),
        ActivatedAbilityKind::Dark(kind) => activate_with::<crate::rules::dark::ActivatedProvider>(
            &offer_context,
            &resolve_context,
            kind,
        ),
    }
}

fn activate_with<P>(
    offer_context: &OfferContext<'_>,
    resolve_context: &ResolveContext<'_>,
    kind: P::Kind,
) -> GameResult<Vec<GameEvent>>
where
    P: ActivatedAbilityProvider,
{
    let offered = P::offers(offer_context)?;
    if !offered
        .iter()
        .any(|offer| offer.kind == kind && offer.completion.matches(resolve_context.input))
    {
        return cannot_resolve(P::id(kind));
    }

    let builder = AbilityPlanBuilder::new(
        resolve_context.state,
        resolve_context.player,
        P::id(kind),
        resolve_context.selection,
    );
    let plan = P::resolve(resolve_context, kind, builder).map_err(|error| match error {
        GameError::Validation(ValidationError::UnknownProfessionAbility(_)) => {
            GameError::RuleImplementation(RuleImplementationError::EffectNotImplemented(
                P::id(kind).to_string(),
            ))
        }
        other => other,
    })?;
    let mut events = vec![GameEvent::ProfessionAbilityActivated {
        player: resolve_context.player.clone(),
        ability_id: P::id(kind).to_string(),
        prepared: plan.prepared,
    }];
    events.extend(plan.consequences);
    if let Some(continuation) = plan.continuation {
        events.push(match continuation {
            AbilityContinuation::Choice(event) | AbilityContinuation::Randomness(event) => event,
        });
    }
    Ok(events)
}

fn parse(id: &str) -> Option<ActivatedAbilityKind> {
    let parsed = [
        crate::rules::hero::ActivatedProvider::parse(id).map(ActivatedAbilityKind::Hero),
        crate::rules::jianghu::ActivatedProvider::parse(id).map(ActivatedAbilityKind::Jianghu),
        crate::rules::confluence::ActivatedProvider::parse(id)
            .map(ActivatedAbilityKind::Confluence),
        crate::rules::dark::ActivatedProvider::parse(id).map(ActivatedAbilityKind::Dark),
    ];
    let mut matches = parsed.into_iter().flatten();
    let first = matches.next()?;
    debug_assert!(matches.next().is_none(), "Activated Ability ID 不可重複");
    Some(first)
}

fn ensure_available(
    state: &GameState,
    player: &PlayerId,
    kind: ActivatedAbilityKind,
    ability_id: &str,
) -> GameResult<()> {
    let (module_id, catalog_id) = match kind {
        ActivatedAbilityKind::Hero(kind) => (
            crate::rules::hero::ActivatedProvider::MODULE_ID,
            crate::rules::hero::ActivatedProvider::catalog_id(kind),
        ),
        ActivatedAbilityKind::Jianghu(kind) => (
            crate::rules::jianghu::ActivatedProvider::MODULE_ID,
            crate::rules::jianghu::ActivatedProvider::catalog_id(kind),
        ),
        ActivatedAbilityKind::Confluence(kind) => (
            crate::rules::confluence::ActivatedProvider::MODULE_ID,
            crate::rules::confluence::ActivatedProvider::catalog_id(kind),
        ),
        ActivatedAbilityKind::Dark(kind) => (
            crate::rules::dark::ActivatedProvider::MODULE_ID,
            crate::rules::dark::ActivatedProvider::catalog_id(kind),
        ),
    };
    if !state.has_rule_module(module_id) {
        return if module_id == crate::domain::HERO_SCHOOLS_MODULE_ID {
            Err(GameError::Validation(ValidationError::HeroSchoolsDisabled))
        } else {
            unavailable(ability_id)
        };
    }
    if crate::rules::pouch::profession_is_suppressed(state, player) {
        return unavailable(ability_id);
    }
    let profession = state
        .profession_for(player)
        .ok_or_else(|| unavailable_error(ability_id))?;
    if !super::effective_ability_ids(&state.enabled_rule_modules, profession).contains(&catalog_id)
    {
        return unavailable(ability_id);
    }
    Ok(())
}

fn presentation_detail(
    state: &GameState,
    player: &PlayerId,
    selection: &ValidatedAbilitySelection,
    presentation: AbilityPresentation,
) -> PlayerFacingActionDetail {
    let mut consequences = vec![RuleConsequence::ImmediateEffect {
        certainty: ConsequenceCertainty::Guaranteed,
        effect: ImmediateEffect::ActivateProfessionAbility {
            effect: presentation.effect,
        },
    }];
    if presentation.discards_selected_cards && !selection.is_empty() {
        consequences.push(RuleConsequence::Cost {
            certainty: ConsequenceCertainty::Guaranteed,
            cost: ActionCost::DiscardSelectedCards,
        });
    }
    if let Some(key) = presentation.limited_use_key
        && let Some(use_count) = state
            .limited_uses
            .iter()
            .find(|use_count| use_count.owner == *player && use_count.key == key)
    {
        consequences.push(RuleConsequence::RuleException {
            certainty: ConsequenceCertainty::Guaranteed,
            exception: RuleException::LimitedUse {
                key: use_count.key.clone(),
                remaining: use_count.remaining,
                maximum: use_count.maximum,
            },
        });
    }
    PlayerFacingActionDetail::composed(consequences)
}

fn ability_was_activated_this_turn(state: &GameState, player: &PlayerId) -> bool {
    state
        .activated_profession_ability_turns
        .get(player)
        .is_some_and(|turn| *turn == state.turn_number)
}

fn cannot_resolve<T>(ability_id: &str) -> GameResult<T> {
    Err(GameError::Validation(
        ValidationError::ProfessionAbilityCannotResolve(ability_id.to_string()),
    ))
}

fn unavailable<T>(ability_id: &str) -> GameResult<T> {
    Err(unavailable_error(ability_id))
}

fn unavailable_error(ability_id: &str) -> GameError {
    GameError::Validation(ValidationError::ProfessionAbilityUnavailable(
        ability_id.to_string(),
    ))
}

fn invalid_plan() -> GameError {
    GameError::EngineInvariant(EngineInvariantError::InvalidProfessionAbilityPlan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ChoiceId, ChoiceRequest, GameSetup, PendingChoice, PendingChoiceKind, PendingResolution,
    };

    #[test]
    fn provider_ids_are_unique_and_round_trip() {
        let mut ids = HashSet::new();
        check_provider::<crate::rules::hero::ActivatedProvider>(&mut ids);
        check_provider::<crate::rules::jianghu::ActivatedProvider>(&mut ids);
        check_provider::<crate::rules::confluence::ActivatedProvider>(&mut ids);
        check_provider::<crate::rules::dark::ActivatedProvider>(&mut ids);
    }

    fn check_provider<P: ActivatedAbilityProvider>(ids: &mut HashSet<&'static str>) {
        for kind in P::kinds() {
            let id = P::id(*kind);
            assert!(ids.insert(id), "duplicate Activated Ability ID: {id}");
            assert_eq!(P::parse(id), Some(*kind));
        }
    }

    #[test]
    fn builder_keeps_one_final_continuation_with_the_matching_medium() {
        let player = PlayerId::new("p1");
        let mut state = GameState::from_setup(&GameSetup::two_player(
            player.clone(),
            PlayerId::new("p2"),
            100,
        ));
        let card = CardInstanceId::new(1);
        state.hand_mut(&player).unwrap().push(card);
        let selection = ValidatedAbilitySelection::new(&state, &player, &[card]).unwrap();
        let mut builder = AbilityPlanBuilder::new(&state, &player, "test", &selection);
        builder
            .push_consequence(GameEvent::TurnDrawBonusChanged {
                player: player.clone(),
                old_value: 0,
                delta: 1,
                new_value: 1,
            })
            .unwrap();
        builder
            .set_continuation(choice_event(
                &state,
                &player,
                PendingResolution::HeroRevelationKeepOne,
            ))
            .unwrap();

        assert!(matches!(
            builder.push_consequence(GameEvent::TurnDrawBonusChanged {
                player: player.clone(),
                old_value: 1,
                delta: 1,
                new_value: 2,
            }),
            Err(GameError::EngineInvariant(
                EngineInvariantError::InvalidProfessionAbilityPlan
            ))
        ));
        assert!(matches!(
            builder.set_continuation(choice_event(
                &state,
                &player,
                PendingResolution::HeroRevelationKeepOne,
            )),
            Err(GameError::EngineInvariant(
                EngineInvariantError::InvalidProfessionAbilityPlan
            ))
        ));
        let plan = builder.finish();
        assert_eq!(plan.consequences.len(), 1);
        assert!(matches!(
            plan.continuation,
            Some(AbilityContinuation::Choice(_))
        ));
    }

    #[test]
    fn builder_rejects_a_continuation_with_the_wrong_waiting_medium() {
        let player = PlayerId::new("p1");
        let state = GameState::from_setup(&GameSetup::two_player(
            player.clone(),
            PlayerId::new("p2"),
            100,
        ));
        let selection = ValidatedAbilitySelection { cards: Vec::new() };
        let mut builder = AbilityPlanBuilder::new(&state, &player, "test", &selection);

        assert!(matches!(
            builder.set_continuation(choice_event(
                &state,
                &player,
                PendingResolution::HeroRevelation,
            )),
            Err(GameError::EngineInvariant(
                EngineInvariantError::InvalidProfessionAbilityPlan
            ))
        ));
    }

    fn choice_event(
        _state: &GameState,
        player: &PlayerId,
        resolution: PendingResolution,
    ) -> GameEvent {
        let request = ChoiceRequest {
            player: player.clone(),
            kind: PendingChoiceKind::Card {
                cards: Vec::new(),
                minimum: 0,
                maximum: 0,
                can_decline: false,
            },
            resolution: resolution.clone(),
        };
        GameEvent::ChoiceRequested {
            choice: PendingChoice {
                choice_id: ChoiceId::new(1),
                player: request.player,
                kind: request.kind,
            },
            resolution,
        }
    }
}

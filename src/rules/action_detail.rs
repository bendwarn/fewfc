use crate::domain::{CardDef, CardInstanceId, Element, GameState, StarElementSubstitution};

pub(crate) fn formation_summary(
    state: &GameState,
    formation_id: &str,
    rule_text: &str,
    star_substitution: Option<&StarElementSubstitution>,
) -> String {
    let base = echo_action_detail(formation_id, rule_text).unwrap_or_else(|| rule_text.to_string());
    star_substitution.map_or(base.clone(), |substitution| {
        format!(
            "{} 星辰效果：將{}（{}）視為{}。",
            base,
            card_summary(state, substitution.card),
            card_element_name(substitution.printed_element),
            card_element_name(substitution.interpreted_element),
        )
    })
}

fn echo_action_detail(formation_id: &str, rule_text: &str) -> Option<String> {
    let main_effect = punctuated_rule_text(rule_text);
    let detail = match formation_id {
        crate::rules::echo::RINGING_METAL => format!(
            "{} 主效果完整結算後，可捨棄一張印刷行屬為金或土的手牌作為迴響代價；若支付，於自己下次回合開始只再次執行此曲調主效果，不視為新的陣法，不會再次排定迴響。",
            main_effect
        ),
        crate::rules::echo::FALLING_WOOD => format!(
            "{} 主效果完整結算後，可捨棄一張印刷行屬為木或水的手牌作為迴響代價；若支付，於自己下次回合開始只再次執行此曲調主效果，不視為新的陣法，不會再次排定迴響。",
            main_effect
        ),
        crate::rules::echo::FLOWING_WATER => format!(
            "{} 主效果完整結算後，可捨棄一張印刷行屬為水或金的手牌作為迴響代價；若支付，於自己下次回合開始只再次執行此曲調主效果，不視為新的陣法，不會再次排定迴響。",
            main_effect
        ),
        crate::rules::echo::WAR_FIRE => format!(
            "{} 主效果完整結算後，可捨棄一張印刷行屬為火或木的手牌作為迴響代價；若支付，於自己下次回合開始只再次執行此曲調主效果，不視為新的陣法，不會再次排定迴響。",
            main_effect
        ),
        crate::rules::echo::SPLIT_EARTH => format!(
            "{} 主效果完整結算後，可捨棄一張印刷行屬為土或火的手牌作為迴響代價；若支付，於自己下次回合開始只再次執行此曲調主效果，不視為新的陣法，不會再次排定迴響。",
            main_effect
        ),
        crate::rules::echo::PURE_FIRE => format!(
            "{} 主效果完整結算後，不需支付迴響代價並自動排定迴響；於自己下次回合開始重新選擇玩家，只再次執行此主效果，不視為新的陣法，不會再次排定迴響。",
            main_effect
        ),
        crate::rules::echo::PLANT_EARTH => format!(
            "{} 這不是迴響；排定自己下次回合開始選擇鳴金、落木、流水、戰火或裂土之一並只執行其主效果，不支付迴響代價、不排定迴響、不視為新的陣法。",
            main_effect
        ),
        _ => return None,
    };
    Some(detail)
}

fn punctuated_rule_text(rule_text: &str) -> String {
    if rule_text.ends_with('。') {
        rule_text.to_string()
    } else {
        format!("{rule_text}。")
    }
}

fn card_summary(state: &GameState, card: CardInstanceId) -> String {
    state
        .card_def(card)
        .map(card_label)
        .unwrap_or_else(|| "一張牌".to_string())
}

fn card_label(card: &CardDef) -> String {
    format!("{} {}", element_short_name(card.element), card.level)
}

fn element_short_name(element: Element) -> &'static str {
    match element {
        Element::Metal => "金",
        Element::Wood => "木",
        Element::Water => "水",
        Element::Fire => "火",
        Element::Earth => "土",
    }
}

fn card_element_name(element: Element) -> &'static str {
    match element {
        Element::Metal => "金行牌",
        Element::Wood => "木行牌",
        Element::Water => "水行牌",
        Element::Fire => "火行牌",
        Element::Earth => "土行牌",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CardDefId, CardInstanceDef, GameSetup, PlayerId};

    #[test]
    fn echo_formation_summary_includes_echo_policy() {
        let setup = GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30);
        let state = GameState::from_setup(&setup);

        let summary = formation_summary(
            &state,
            crate::rules::echo::FALLING_WOOD,
            "木木；自身隊伍回復１５點生命",
            None,
        );

        assert!(summary.contains("木木；自身隊伍回復１５點生命"));
        assert!(summary.contains("印刷行屬為木或水"));
        assert!(summary.contains("自己下次回合開始"));
        assert!(summary.contains("只再次執行此曲調主效果"));
        assert!(summary.contains("不視為新的陣法"));
        assert!(summary.contains("不會再次排定迴響"));

        let pure_fire = formation_summary(
            &state,
            crate::rules::echo::PURE_FIRE,
            "火水且等級合計７以上；指定玩家的合格時效效果減少１回合或１層",
            None,
        );
        assert!(pure_fire.contains("不需支付迴響代價並自動排定迴響"));
        assert!(pure_fire.contains("重新選擇玩家"));
        assert!(pure_fire.contains("不視為新的陣法"));

        let plant_earth = formation_summary(
            &state,
            crate::rules::echo::PLANT_EARTH,
            "土木且等級合計７以上；下次自己回合開始選擇一種基礎曲調主效果",
            None,
        );
        assert!(plant_earth.contains("這不是迴響"));
        assert!(plant_earth.contains("選擇鳴金、落木、流水、戰火或裂土"));
        assert!(plant_earth.contains("不支付迴響代價"));
        assert!(plant_earth.contains("不排定迴響"));
        assert!(plant_earth.contains("不視為新的陣法"));
    }

    #[test]
    fn formation_summary_composes_with_star_substitution_detail() {
        let mut setup = GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30);
        setup.card_defs.push(CardDef {
            id: CardDefId::new("water-3"),
            name: "水 3".to_string(),
            element: Element::Water,
            level: 3,
        });
        setup.card_instances.push(CardInstanceDef {
            instance: CardInstanceId::new(7),
            definition: CardDefId::new("water-3"),
            origin: Default::default(),
        });
        let state = GameState::from_setup(&setup);
        let substitution = StarElementSubstitution {
            card: CardInstanceId::new(7),
            printed_element: Element::Water,
            interpreted_element: Element::Wood,
        };

        let summary = formation_summary(
            &state,
            crate::rules::echo::FALLING_WOOD,
            "木木；自身隊伍回復１５點生命",
            Some(&substitution),
        );

        assert!(summary.contains("印刷行屬為木或水"));
        assert!(summary.contains("不會再次排定迴響"));
        assert!(summary.contains("星辰效果：將水 3（水行牌）視為木行牌。"));
    }
}

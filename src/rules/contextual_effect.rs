use super::no_set_state_in_effect;
use crate::{
    ast::excerpt,
    rule::{Rule, RuleContext},
};
use oxc_semantic::Semantic;

pub struct ContextualEffect;
impl Rule for ContextualEffect {
    fn id(&self) -> &'static str {
        "contextual-effect"
    }
    fn check(&self, semantic: &Semantic<'_>, context: &mut RuleContext<'_>) {
        for (effect, function) in no_set_state_in_effect::candidates(semantic) {
            context.request_model_with_message(effect,
                "useEffect 내부 상태 업데이트를 검사하세요. 외부 구독으로 전달받은 값을 반영하는 경우는 pass, 그 외의 상태 업데이트는 violation입니다. 문맥이 부족하면 unknown을 반환하세요.",
                excerpt(semantic.source_text(), function), no_set_state_in_effect::MESSAGE);
        }
    }
}

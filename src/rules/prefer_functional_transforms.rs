use crate::{
    ast::{contains, excerpt, local_function, loop_body, reference_symbol},
    rule::{Rule, RuleContext},
};
use oxc_ast::{
    AstKind,
    ast::{BindingPattern, Expression},
};
use oxc_semantic::Semantic;
use oxc_span::{GetSpan, Span};
use std::collections::{BTreeMap, HashMap};

pub struct PreferFunctionalTransforms;
pub const MESSAGE: &str = "순수한 컬렉션 변환에는 명확한 함수형 표현을 사용하세요.";
const CRITERIA: &str = r#"
지역 빈 배열에 반복문으로 push하는 후보 함수입니다.
입력 컬렉션의 변환·선별을 map, filter, flatMap 등으로 기존 동작을 유지하면서
더 명확히 표현할 수 있으면 violation입니다.

부수 효과의 순서, 조기 종료, 성능 요구 때문에 반복문이 적절하거나
복잡한 reduce·과도한 중간 배열이 필요하면 pass입니다.

지역 배열 변경만으로 비순수하다고 판단하지 마세요.
반환값·오류 처리·실행 순서를 보존해야 합니다.
전체 실패를 filter의 부분 성공으로 바꾸지 마세요.
문맥이 부족하면 unknown을 반환하세요.
"#;

impl Rule for PreferFunctionalTransforms {
    fn id(&self) -> &'static str {
        "prefer-functional-transforms"
    }
    fn check(&self, semantic: &Semantic<'_>, context: &mut RuleContext<'_>) {
        for function in candidates(semantic) {
            context.request_model_with_message(
                function,
                CRITERIA,
                excerpt(semantic.source_text(), function),
                MESSAGE,
            );
        }
    }
}

struct LocalArray {
    declaration: Span,
    function: Span,
}
fn local_empty_arrays(semantic: &Semantic<'_>) -> HashMap<usize, LocalArray> {
    semantic
        .nodes()
        .iter()
        .filter_map(|node| {
            let AstKind::VariableDeclarator(declaration) = node.kind() else {
                return None;
            };
            let BindingPattern::BindingIdentifier(id) = &declaration.id else {
                return None;
            };
            let Expression::ArrayExpression(array) =
                declaration.init.as_ref()?.get_inner_expression()
            else {
                return None;
            };
            if !array.elements.is_empty() {
                return None;
            }
            Some((
                id.symbol_id.get()?.index(),
                LocalArray {
                    declaration: declaration.span,
                    function: local_function(semantic, node)?,
                },
            ))
        })
        .collect()
}

/// Find direct pushes into a local array declared before the containing loop.
/// A candidate is not a proof of purity; the model judges refactoring suitability.
pub(super) fn candidates(semantic: &Semantic<'_>) -> Vec<Span> {
    let arrays = local_empty_arrays(semantic);
    let mut functions = BTreeMap::new();
    for node in semantic.nodes().iter() {
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        let Expression::StaticMemberExpression(member) = call.callee.get_inner_expression() else {
            continue;
        };
        if call.optional
            || member.optional
            || call.arguments.is_empty()
            || member.property.name != "push"
        {
            continue;
        }
        let Some(array) =
            reference_symbol(semantic, &member.object).and_then(|symbol| arrays.get(&symbol))
        else {
            continue;
        };
        if local_function(semantic, node) != Some(array.function) {
            continue;
        }
        let in_loop = semantic
            .nodes()
            .ancestor_kinds(node.id())
            .take_while(|kind| !crate::ast::is_function(*kind))
            .any(|kind| {
                loop_body(kind).is_some_and(|body| {
                    contains(body, call.span) && array.declaration.end <= kind.span().start
                })
            });
        if in_loop {
            functions.insert(array.function.start, array.function);
        }
    }
    functions.into_values().collect()
}

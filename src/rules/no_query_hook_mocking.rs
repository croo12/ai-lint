//! Keep data hooks real in tests; replace network responses with MSW.
mod facts;
#[cfg(test)]
mod tests;

use crate::rule::{Rule, RuleContext};
use facts::{Facts, Hook, argument, is_query_library, member, module_hook_name};
use oxc_ast::{AstKind, ast::CallExpression};
use oxc_semantic::Semantic;

pub struct NoQueryHookMocking;

impl Rule for NoQueryHookMocking {
    fn id(&self) -> &'static str {
        "no-query-hook-mocking"
    }

    fn check(&self, semantic: &Semantic<'_>, context: &mut RuleContext<'_>) {
        let is_test = context
            .file_path()
            .and_then(|p| p.file_name())
            .and_then(|p| p.to_str())
            .is_some_and(|name| name.ends_with(".test.ts") || name.ends_with(".test.tsx"));
        if !is_test {
            return;
        }

        let facts = Facts::new(semantic);
        for node in semantic.nodes().iter() {
            let AstKind::CallExpression(call) = node.kind() else {
                continue;
            };
            if forbidden_call(&facts, call) {
                context.report(call.span,
                    "데이터 요청 hook을 통째로 mock하지 마세요. 실제 hook을 사용하고 MSW handler(server.use + http/graphql)로 네트워크 응답을 제어하세요.");
            }
        }
    }
}

fn forbidden_call(facts: &Facts<'_, '_>, call: &CallExpression<'_>) -> bool {
    let Some((object, method)) = member(&call.callee) else {
        return false;
    };
    if facts.is_framework(object) {
        return match method {
            "mock" | "doMock" | "unstable_mockModule" => module_mock(facts, call),
            "spyOn" | "replaceProperty" => facts.spy_hook(call).is_some_and(|hook| hook.is_query()),
            _ => false,
        };
    }
    if !matches!(
        method,
        "mockReturnValue"
            | "mockReturnValueOnce"
            | "mockResolvedValue"
            | "mockResolvedValueOnce"
            | "mockRejectedValue"
            | "mockRejectedValueOnce"
            | "mockImplementation"
            | "mockImplementationOnce"
            | "withImplementation"
    ) {
        return false;
    }
    let Some(hook) = facts.hook(object, 0) else {
        return false;
    };
    // A spy already reports its own location; avoid two reports for one chain.
    if facts.is_query_spy(object) {
        return false;
    }
    hook.is_query() || argument(call, 0).is_some_and(|value| facts.query_result(value))
}

fn module_mock(facts: &Facts<'_, '_>, call: &CallExpression<'_>) -> bool {
    let Some(source) = argument(call, 0).and_then(facts::module_source) else {
        return false;
    };
    let Some(factory) = argument(call, 1) else {
        return auto_mock(facts, source);
    };
    // Vitest's { spy: true } replaces the module exports without a factory.
    if facts.spy_option(factory) {
        return auto_mock(facts, source);
    }
    facts.objects(factory, 0).iter().any(|object| {
        object
            .properties
            .iter()
            .filter_map(|property| property.as_property())
            .any(|property| {
                let Some(name) = property.key.static_name() else {
                    return false;
                };
                let hook_name = if name == "default" {
                    module_hook_name(source)
                } else {
                    name.into_owned()
                };
                let Some(hook) = Hook::new(hook_name, Some(source.to_owned())) else {
                    return false;
                };
                !facts.passthrough(&property.value, &hook, factory)
                    && (hook.is_query() || facts.query_result(&property.value))
            })
    })
}

fn auto_mock(facts: &Facts<'_, '_>, source: &str) -> bool {
    is_query_library(source)
        || Hook::new(module_hook_name(source), Some(source.to_owned()))
            .is_some_and(|hook| hook.is_query())
        || facts.imports_query_hook(source)
}

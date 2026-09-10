//! File-local binding and mock-factory analysis. No project traversal or model calls.
use crate::ast::{enclosing_function, reference_symbol};
use oxc_ast::{
    AstKind,
    ast::{CallExpression, Expression, ImportDeclarationSpecifier, ObjectExpression},
};
use oxc_semantic::Semantic;
use std::collections::HashMap;

const MAX_ALIAS_DEPTH: usize = 12;

pub(super) struct Hook {
    name: String,
    module: Option<String>,
}
impl Hook {
    pub(super) fn new(name: String, module: Option<String>) -> Option<Self> {
        (name.starts_with("use") && name.as_bytes().get(3).is_some_and(u8::is_ascii_uppercase))
            .then_some(Self { name, module })
    }
    pub(super) fn is_query(&self) -> bool {
        self.module.as_deref().is_some_and(is_api_module)
            || self.name.starts_with("useSuspense")
            || matches!(
                self.name.as_str(),
                "useSWR" | "useSWRInfinite" | "useSWRMutation" | "useMutationState"
            )
            || ["Query", "Queries", "Mutation", "Subscription"]
                .iter()
                .any(|suffix| self.name.ends_with(suffix))
    }
}

pub(super) struct Facts<'s, 'a> {
    semantic: &'s Semantic<'a>,
    returns: HashMap<u32, Vec<&'a Expression<'a>>>,
}
impl<'s, 'a> Facts<'s, 'a> {
    pub(super) fn new(semantic: &'s Semantic<'a>) -> Self {
        let mut returns: HashMap<u32, Vec<&Expression<'_>>> = HashMap::new();
        for node in semantic.nodes().iter() {
            if let AstKind::ReturnStatement(statement) = node.kind()
                && let Some(value) = &statement.argument
                && let Some(function) = enclosing_function(semantic, node)
            {
                returns.entry(function.start).or_default().push(value);
            }
        }
        Self { semantic, returns }
    }

    fn declaration(&self, expression: &Expression<'_>) -> Option<AstKind<'a>> {
        let Expression::Identifier(id) = expression.get_inner_expression() else {
            return None;
        };
        let symbol = self
            .semantic
            .scoping()
            .get_reference(id.reference_id.get()?)
            .symbol_id()?;
        Some(
            self.semantic
                .nodes()
                .kind(self.semantic.scoping().symbol_declaration(symbol)),
        )
    }

    fn import_source(&self, expression: &Expression<'_>) -> Option<String> {
        let Expression::Identifier(id) = expression.get_inner_expression() else {
            return None;
        };
        let symbol = self
            .semantic
            .scoping()
            .get_reference(id.reference_id.get()?)
            .symbol_id()?;
        self.semantic
            .nodes()
            .ancestor_kinds(self.semantic.scoping().symbol_declaration(symbol))
            .find_map(|kind| match kind {
                AstKind::ImportDeclaration(import) => Some(import.source.value.to_string()),
                _ => None,
            })
    }

    pub(super) fn is_framework(&self, expression: &Expression<'_>) -> bool {
        let Expression::Identifier(id) = expression.get_inner_expression() else {
            return false;
        };
        match self.declaration(expression) {
            Some(AstKind::ImportSpecifier(import)) => {
                matches!(import.imported.name().as_str(), "vi" | "vitest" | "jest")
                    && self
                        .import_source(expression)
                        .is_some_and(|source| matches!(source.as_str(), "vitest" | "@jest/globals"))
            }
            None => matches!(id.name.as_str(), "vi" | "vitest" | "jest"),
            _ => false,
        }
    }

    pub(super) fn hook(&self, expression: &Expression<'_>, depth: usize) -> Option<Hook> {
        if depth > MAX_ALIAS_DEPTH {
            return None;
        }
        match expression.get_inner_expression() {
            Expression::Identifier(id) => match self.declaration(expression) {
                Some(AstKind::ImportSpecifier(import)) => Hook::new(
                    import.imported.name().to_string(),
                    self.import_source(expression),
                ),
                Some(AstKind::ImportDefaultSpecifier(_)) => {
                    let source = self.import_source(expression)?;
                    let module_name = module_hook_name(&source);
                    let name = if Hook::new(module_name.clone(), None).is_some() {
                        module_name
                    } else {
                        id.name.to_string()
                    };
                    Hook::new(name, Some(source))
                }
                Some(AstKind::VariableDeclarator(var)) if var.init.is_some() => self
                    .hook(var.init.as_ref()?, depth + 1)
                    .or_else(|| Hook::new(id.name.to_string(), None)),
                _ => Hook::new(id.name.to_string(), None),
            },
            Expression::CallExpression(call) => {
                let (object, method) = member(&call.callee)?;
                if !self.is_framework(object) {
                    return None;
                }
                match method {
                    "mocked" => self.hook(argument(call, 0)?, depth + 1),
                    "spyOn" => self.spy_hook(call),
                    _ => None,
                }
            }
            expression => {
                let (object, name) = member(expression)?;
                Hook::new(name.to_owned(), self.import_source(object))
            }
        }
    }

    pub(super) fn spy_hook(&self, call: &CallExpression<'_>) -> Option<Hook> {
        let Expression::StringLiteral(name) = argument(call, 1)?.get_inner_expression() else {
            return None;
        };
        Hook::new(
            name.value.to_string(),
            self.import_source(argument(call, 0)?),
        )
    }

    pub(super) fn is_query_spy(&self, expression: &Expression<'_>) -> bool {
        let Expression::CallExpression(call) = expression.get_inner_expression() else {
            return false;
        };
        member(&call.callee)
            .is_some_and(|(object, name)| name == "spyOn" && self.is_framework(object))
            && self.spy_hook(call).is_some_and(|hook| hook.is_query())
    }

    pub(super) fn imports_query_hook(&self, source: &str) -> bool {
        self.semantic.nodes().iter().any(|node| {
            let AstKind::ImportDeclaration(import) = node.kind() else {
                return false;
            };
            import.source.value == source
                && import.specifiers.iter().flatten().any(|specifier| {
                    let name = match specifier {
                        ImportDeclarationSpecifier::ImportSpecifier(import) => {
                            import.imported.name().to_string()
                        }
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(import) => {
                            import.local.name.to_string()
                        }
                        _ => return false,
                    };
                    Hook::new(name, Some(source.to_owned())).is_some_and(|hook| hook.is_query())
                })
        })
    }

    /// Follow only returned values, never arbitrary nested objects or callbacks.
    pub(super) fn objects<'e>(
        &'e self,
        expression: &'e Expression<'a>,
        depth: usize,
    ) -> Vec<&'e ObjectExpression<'a>> {
        if depth > MAX_ALIAS_DEPTH {
            return Vec::new();
        }
        match expression.get_inner_expression() {
            Expression::ObjectExpression(object) => vec![object],
            Expression::Identifier(_) => match self.declaration(expression) {
                Some(AstKind::VariableDeclarator(var)) => var
                    .init
                    .as_ref()
                    .map(|value| self.objects(value, depth + 1))
                    .unwrap_or_default(),
                _ => Vec::new(),
            },
            Expression::ArrowFunctionExpression(function) => match function.body.as_expression() {
                Some(body) => self.objects(body, depth + 1),
                None => self.returned_objects(function.span.start, depth),
            },
            Expression::FunctionExpression(function) => {
                self.returned_objects(function.span.start, depth)
            }
            Expression::CallExpression(call) => {
                let Some((object, method)) = member(&call.callee) else {
                    return Vec::new();
                };
                if (method == "fn" && self.is_framework(object))
                    || matches!(
                        method,
                        "mockReturnValue"
                            | "mockReturnValueOnce"
                            | "mockImplementation"
                            | "mockImplementationOnce"
                    )
                {
                    return argument(call, 0)
                        .map(|value| self.objects(value, depth + 1))
                        .unwrap_or_default();
                }
                Vec::new()
            }
            Expression::ConditionalExpression(conditional) => self
                .objects(&conditional.consequent, depth + 1)
                .into_iter()
                .chain(self.objects(&conditional.alternate, depth + 1))
                .collect(),
            _ => Vec::new(),
        }
    }

    fn returned_objects(&self, start: u32, depth: usize) -> Vec<&ObjectExpression<'a>> {
        self.returns
            .get(&start)
            .into_iter()
            .flatten()
            .flat_map(|value| self.objects(value, depth + 1))
            .collect()
    }

    pub(super) fn query_result(&self, expression: &Expression<'a>) -> bool {
        self.objects(expression, 0).iter().any(|object| {
            let keys: Vec<_> = object
                .properties
                .iter()
                .filter_map(|property| property.as_property()?.key.static_name())
                .collect();
            let has = |key: &str| keys.iter().any(|name| name == key);
            has("mutate")
                || has("mutateAsync")
                || has("refetch")
                || has("fetchNextPage")
                || (has("data")
                    && [
                        "isLoading",
                        "isPending",
                        "isError",
                        "isSuccess",
                        "error",
                        "status",
                        "isFetching",
                    ]
                    .iter()
                    .any(|key| has(key)))
        })
    }

    pub(super) fn spy_option(&self, expression: &Expression<'_>) -> bool {
        let Expression::ObjectExpression(object) = expression.get_inner_expression() else {
            return false;
        };
        object.properties.iter().any(|property| property.as_property().is_some_and(|property|
            property.key.static_name().is_some_and(|name| name == "spy")
                && matches!(property.value.get_inner_expression(), Expression::BooleanLiteral(value) if value.value)))
    }

    pub(super) fn passthrough(
        &self,
        expression: &Expression<'_>,
        hook: &Hook,
        factory: &Expression<'_>,
    ) -> bool {
        if let Some(imported) = self.hook(expression, 0)
            && imported.name == hook.name
            && imported.module.is_some()
            && imported.module == hook.module
        {
            return true;
        }
        let Some((object, name)) = member(expression) else {
            return false;
        };
        if name != hook.name {
            return false;
        }
        let Some(AstKind::VariableDeclarator(var)) = self.declaration(object) else {
            return false;
        };
        let Some(value) = &var.init else {
            return false;
        };
        let value = match value.get_inner_expression() {
            Expression::AwaitExpression(value) => value.argument.get_inner_expression(),
            value => value,
        };
        let Expression::CallExpression(call) = value else {
            return false;
        };
        match call.callee.get_inner_expression() {
            Expression::Identifier(_) => self.is_factory_importer(&call.callee, factory),
            value => member(value).is_some_and(|(object, name)| {
                self.is_framework(object)
                    && matches!(name, "importActual" | "requireActual")
                    && argument(call, 0).and_then(module_source) == hook.module.as_deref()
            }),
        }
    }

    fn is_factory_importer(&self, callee: &Expression<'_>, factory: &Expression<'_>) -> bool {
        let params = match factory.get_inner_expression() {
            Expression::ArrowFunctionExpression(function) => &function.params,
            Expression::FunctionExpression(function) => &function.params,
            _ => return false,
        };
        let Some(binding) = params
            .items
            .first()
            .and_then(|param| param.pattern.get_binding_identifier())
        else {
            return false;
        };
        binding
            .symbol_id
            .get()
            .is_some_and(|symbol| reference_symbol(self.semantic, callee) == Some(symbol.index()))
    }
}

pub(super) fn member<'e, 'a>(
    expression: &'e Expression<'a>,
) -> Option<(&'e Expression<'a>, &'a str)> {
    let member = expression.get_inner_expression().as_member_expression()?;
    Some((member.object(), member.static_property_name()?))
}
pub(super) fn argument<'e, 'a>(
    call: &'e CallExpression<'a>,
    index: usize,
) -> Option<&'e Expression<'a>> {
    call.arguments.get(index)?.as_expression()
}
pub(super) fn module_source<'a>(expression: &Expression<'a>) -> Option<&'a str> {
    match expression.get_inner_expression() {
        Expression::StringLiteral(source) => Some(source.value.as_str()),
        Expression::ImportExpression(import) => module_source(&import.source),
        _ => None,
    }
}
pub(super) fn is_api_module(source: &str) -> bool {
    source
        .split('/')
        .any(|segment| segment == "api" || segment == "@api")
}
pub(super) fn is_query_library(source: &str) -> bool {
    matches!(
        source,
        "@tanstack/react-query"
            | "react-query"
            | "@apollo/client"
            | "@apollo/client/react"
            | "swr"
            | "swr/infinite"
            | "swr/mutation"
    )
}
pub(super) fn module_hook_name(source: &str) -> String {
    if source == "swr" || source.starts_with("swr/") {
        return "useSWR".into();
    }
    let name = source
        .rsplit('/')
        .next()
        .unwrap_or(source)
        .split('.')
        .next()
        .unwrap_or(source);
    let mut parts = name.split('-');
    let mut name = parts.next().unwrap_or_default().to_owned();
    for part in parts {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            name.extend(first.to_uppercase());
            name.extend(chars);
        }
    }
    name
}

//! ESTree provides open-ended property traversal; Oxc resolves lexical bindings.
use oxc_ast::{AstKind, ast::Program};
use oxc_semantic::SemanticBuilder;
use oxc_span::Span;
use serde_json::Value;
use std::collections::HashMap;

pub(crate) struct Node<'a> {
    pub value: &'a Value,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
}
pub(crate) struct Document<'a> {
    pub source: &'a str,
    pub nodes: Vec<Node<'a>>,
    by_address: HashMap<usize, usize>,
    bindings: HashMap<u32, usize>,
}
pub(crate) fn binding_symbols(program: &Program<'_>) -> Result<HashMap<u32, usize>, String> {
    let built = SemanticBuilder::new().with_build_nodes(true).build(program);
    if !built.diagnostics.is_empty() {
        return Err(format!("semantic analysis failed: {:?}", built.diagnostics));
    }
    let mut bindings = HashMap::new();
    for node in built.semantic.nodes().iter() {
        let binding = match node.kind() {
            AstKind::BindingIdentifier(id) => id
                .symbol_id
                .get()
                .map(|symbol| (id.span.start, symbol.index())),
            AstKind::IdentifierReference(id) => id
                .reference_id
                .get()
                .and_then(|reference| {
                    built
                        .semantic
                        .scoping()
                        .get_reference(reference)
                        .symbol_id()
                })
                .map(|symbol| (id.span.start, symbol.index())),
            _ => None,
        };
        if let Some((start, symbol)) = binding {
            bindings.insert(start, symbol);
        }
    }
    Ok(bindings)
}
impl<'a> Document<'a> {
    pub fn new(source: &'a str, tree: &'a Value, bindings: HashMap<u32, usize>) -> Self {
        let mut document = Self {
            source,
            nodes: Vec::new(),
            by_address: HashMap::new(),
            bindings,
        };
        document.index(tree, None);
        document
    }
    fn index(&mut self, value: &'a Value, parent: Option<usize>) {
        match value {
            Value::Object(fields) => {
                let parent = if value.get("type").and_then(Value::as_str).is_some()
                    && value.get("start").and_then(Value::as_u64).is_some()
                    && value.get("end").and_then(Value::as_u64).is_some()
                {
                    let id = self.nodes.len();
                    self.nodes.push(Node {
                        value,
                        parent,
                        children: Vec::new(),
                    });
                    self.by_address.insert(value as *const Value as usize, id);
                    if let Some(parent) = parent {
                        self.nodes[parent].children.push(id);
                    }
                    Some(id)
                } else {
                    parent
                };
                for (key, value) in fields {
                    if key != "comments" {
                        self.index(value, parent);
                    }
                }
            }
            Value::Array(values) => {
                for value in values {
                    self.index(value, parent);
                }
            }
            _ => {}
        }
    }
    pub fn node_for(&self, value: &Value) -> Option<usize> {
        self.by_address
            .get(&(value as *const Value as usize))
            .copied()
    }
    pub fn kind(&self, node: usize) -> &str {
        self.nodes[node].value["type"].as_str().unwrap()
    }
    pub fn span(&self, node: usize) -> Span {
        let value = self.nodes[node].value;
        Span::new(
            value["start"].as_u64().unwrap() as u32,
            value["end"].as_u64().unwrap() as u32,
        )
    }
    pub fn excerpt(&self, node: usize) -> &'a str {
        let span = self.span(node);
        &self.source[span.start as usize..span.end as usize]
    }
    pub fn enclosing_function(&self, node: usize) -> usize {
        let mut current = Some(node);
        while let Some(id) = current {
            if matches!(
                self.kind(id),
                "FunctionDeclaration" | "FunctionExpression" | "ArrowFunctionExpression"
            ) {
                return id;
            }
            current = self.nodes[id].parent;
        }
        node
    }
    pub fn binding(&self, value: &Value) -> Option<usize> {
        if value.get("type")?.as_str()? != "Identifier" {
            return None;
        }
        self.bindings
            .get(&(value.get("start")?.as_u64()? as u32))
            .copied()
    }
}
pub(super) fn property<'a>(mut value: &'a Value, path: &str) -> Option<&'a Value> {
    if path == "." {
        return Some(value);
    }
    for part in path.split('.') {
        value = match value {
            Value::Array(values) => values.get(part.parse::<usize>().ok()?)?,
            Value::Object(fields) => fields.get(part)?,
            _ => return None,
        };
    }
    Some(value)
}

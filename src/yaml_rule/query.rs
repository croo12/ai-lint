use super::{Document, YamlRuleError, document::property, invalid};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub(super) type Captures = BTreeMap<String, usize>;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum Names {
    One(String),
    Many(Vec<String>),
}
impl Names {
    fn matches(&self, name: &str) -> bool {
        match self {
            Self::One(value) => value == name,
            Self::Many(values) => values.iter().any(|v| v == name),
        }
    }
    fn validate(&self) -> Result<(), YamlRuleError> {
        match self {
            Self::One(value) if !value.trim().is_empty() => Ok(()),
            Self::Many(values)
                if !values.is_empty() && values.iter().all(|v| !v.trim().is_empty()) =>
            {
                Ok(())
            }
            _ => invalid("node kinds must be a nonempty string or list"),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct Query {
    kind: Option<Names>,
    #[serde(default)]
    properties: BTreeMap<String, Value>,
    #[serde(default)]
    exists: Vec<String>,
    capture: Option<String>,
    same_binding: Option<Comparison>,
    before: Option<Comparison>,
    at: Option<At>,
    child: Option<Relation>,
    descendant: Option<Relation>,
    ancestor: Option<Relation>,
    all: Option<Vec<Query>>,
    any: Option<Vec<Query>>,
    not: Option<Box<Query>>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Comparison {
    left: String,
    right: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct At {
    path: String,
    #[serde(rename = "match")]
    query: Box<Query>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Relation {
    #[serde(rename = "match")]
    query: Box<Query>,
    stop_at: Option<Names>,
}

impl Query {
    pub fn validate(
        &self,
        depth: usize,
        incoming: &BTreeSet<String>,
    ) -> Result<BTreeSet<String>, YamlRuleError> {
        if depth > 32 {
            return invalid("query nesting exceeds 32 levels");
        }
        let navigations = [
            self.at.is_some(),
            self.child.is_some(),
            self.descendant.is_some(),
            self.ancestor.is_some(),
            self.all.is_some(),
            self.any.is_some(),
            self.not.is_some(),
        ]
        .into_iter()
        .filter(|v| *v)
        .count();
        if navigations > 1 {
            return invalid("combine navigation operations explicitly with all/any");
        }
        if navigations == 0
            && self.kind.is_none()
            && self.properties.is_empty()
            && self.exists.is_empty()
            && self.capture.is_none()
            && self.same_binding.is_none()
            && self.before.is_none()
        {
            return invalid("query must not be empty");
        }
        if let Some(kind) = &self.kind {
            kind.validate()?;
        }
        let mut available = incoming.clone();
        if let Some(capture) = &self.capture {
            if !identifier(capture) || !available.insert(capture.clone()) {
                return invalid("capture must have a unique identifier name");
            }
            if available.len() > 32 {
                return invalid("too many captures");
            }
        }
        for path in self.properties.keys().chain(self.exists.iter()) {
            validate_path(path, &available)?;
        }
        for comparison in [&self.same_binding, &self.before].into_iter().flatten() {
            validate_path(&comparison.left, &available)?;
            validate_path(&comparison.right, &available)?;
        }
        if let Some(at) = &self.at {
            validate_path(&at.path, &available)?;
            available = at.query.validate(depth + 1, &available)?;
        }
        if self.child.as_ref().is_some_and(|r| r.stop_at.is_some()) {
            return invalid("child does not accept stop_at");
        }
        for relation in [&self.child, &self.descendant, &self.ancestor]
            .into_iter()
            .flatten()
        {
            if let Some(stop) = &relation.stop_at {
                stop.validate()?;
            }
            available = relation.query.validate(depth + 1, &available)?;
        }
        if let Some(all) = &self.all {
            if all.is_empty() {
                return invalid("all must not be empty");
            }
            for query in all {
                available = query.validate(depth + 1, &available)?;
            }
        }
        if let Some(any) = &self.any {
            if any.is_empty() {
                return invalid("any must not be empty");
            }
            let mut branches = Vec::new();
            for query in any {
                branches.push(query.validate(depth + 1, &available)?);
            }
            available = branches
                .into_iter()
                .reduce(|a, b| a.intersection(&b).cloned().collect())
                .unwrap();
        }
        if let Some(not) = &self.not {
            not.validate(depth + 1, &available)?;
        }
        Ok(available)
    }

    pub fn evaluate(
        &self,
        doc: &Document<'_>,
        node: usize,
        mut captures: Captures,
        budget: &mut usize,
    ) -> Result<Vec<Captures>, String> {
        tick(budget)?;
        if self
            .kind
            .as_ref()
            .is_some_and(|kind| !kind.matches(doc.kind(node)))
        {
            return Ok(vec![]);
        }
        if let Some(name) = &self.capture {
            captures.insert(name.clone(), node);
        }
        if !self
            .properties
            .iter()
            .all(|(path, expected)| resolve(doc, node, &captures, path) == Some(expected))
            || !self
                .exists
                .iter()
                .all(|path| resolve(doc, node, &captures, path).is_some())
        {
            return Ok(vec![]);
        }
        if let Some(comparison) = &self.same_binding {
            let left = resolve(doc, node, &captures, &comparison.left).and_then(|v| doc.binding(v));
            let right =
                resolve(doc, node, &captures, &comparison.right).and_then(|v| doc.binding(v));
            if left.is_none() || left != right {
                return Ok(vec![]);
            }
        }
        if let Some(comparison) = &self.before {
            let left =
                resolve(doc, node, &captures, &comparison.left).and_then(|v| doc.node_for(v));
            let right =
                resolve(doc, node, &captures, &comparison.right).and_then(|v| doc.node_for(v));
            if !left
                .zip(right)
                .is_some_and(|(a, b)| doc.span(a).end <= doc.span(b).start)
            {
                return Ok(vec![]);
            }
        }
        if let Some(at) = &self.at {
            return match resolve(doc, node, &captures, &at.path).and_then(|v| doc.node_for(v)) {
                Some(target) => at.query.evaluate(doc, target, captures, budget),
                None => Ok(vec![]),
            };
        }
        if let Some(relation) = &self.child {
            return relation.search(doc, node, captures, budget, Direction::Child);
        }
        if let Some(relation) = &self.descendant {
            return relation.search(doc, node, captures, budget, Direction::Descendant);
        }
        if let Some(relation) = &self.ancestor {
            return relation.search(doc, node, captures, budget, Direction::Ancestor);
        }
        if let Some(all) = &self.all {
            let mut states = vec![captures];
            for query in all {
                let mut next = Vec::new();
                for state in states {
                    next.extend(query.evaluate(doc, node, state, budget)?);
                }
                states = next;
                if states.is_empty() {
                    break;
                }
            }
            return Ok(states);
        }
        if let Some(any) = &self.any {
            let mut states = Vec::new();
            for query in any {
                states.extend(query.evaluate(doc, node, captures.clone(), budget)?);
            }
            return Ok(states);
        }
        if let Some(not) = &self.not
            && !not
                .evaluate(doc, node, captures.clone(), budget)?
                .is_empty()
        {
            return Ok(vec![]);
        }
        Ok(vec![captures])
    }
}

enum Direction {
    Child,
    Descendant,
    Ancestor,
}
impl Relation {
    fn search(
        &self,
        doc: &Document<'_>,
        node: usize,
        captures: Captures,
        budget: &mut usize,
        direction: Direction,
    ) -> Result<Vec<Captures>, String> {
        let mut pending = match direction {
            Direction::Ancestor => doc.nodes[node].parent.into_iter().collect(),
            _ => doc.nodes[node].children.clone(),
        };
        let mut states = Vec::new();
        while let Some(candidate) = pending.pop() {
            tick(budget)?;
            if self
                .stop_at
                .as_ref()
                .is_some_and(|stop| stop.matches(doc.kind(candidate)))
            {
                continue;
            }
            states.extend(
                self.query
                    .evaluate(doc, candidate, captures.clone(), budget)?,
            );
            match direction {
                Direction::Descendant => {
                    pending.extend(doc.nodes[candidate].children.iter().copied())
                }
                Direction::Ancestor => pending.extend(doc.nodes[candidate].parent),
                Direction::Child => {}
            }
        }
        Ok(states)
    }
}
fn resolve<'a>(
    doc: &Document<'a>,
    node: usize,
    captures: &Captures,
    path: &str,
) -> Option<&'a Value> {
    let (node, path) = if let Some(reference) = path.strip_prefix('$') {
        let (name, path) = reference.split_once('.').unwrap_or((reference, "."));
        (*captures.get(name)?, path)
    } else {
        (node, path)
    };
    property(doc.nodes[node].value, path)
}
fn identifier(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .enumerate()
            .all(|(i, c)| c == b'_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
}
fn validate_path(path: &str, captures: &BTreeSet<String>) -> Result<(), YamlRuleError> {
    if path == "." {
        return Ok(());
    }
    let path = if let Some(reference) = path.strip_prefix('$') {
        let (name, tail) = reference.split_once('.').unwrap_or((reference, "."));
        if !captures.contains(name) {
            return invalid("path refers to an unbound capture");
        }
        if tail == "." {
            return Ok(());
        }
        tail
    } else {
        path
    };
    if path.split('.').all(|part| {
        identifier(part) || (!part.is_empty() && part.bytes().all(|c| c.is_ascii_digit()))
    }) {
        Ok(())
    } else {
        invalid("invalid property path; use dot-separated fields and array indices")
    }
}
fn tick(budget: &mut usize) -> Result<(), String> {
    *budget = budget
        .checked_sub(1)
        .ok_or("query evaluation exceeded 1000000 steps")?;
    Ok(())
}

use super::contract::{Rule, RuleContext};
use oxc_ast::{AstKind, ast::ImportOrExportKind};
use oxc_semantic::Semantic;

pub struct NoMixedTypeExports;

impl Rule for NoMixedTypeExports {
    fn id(&self) -> &'static str {
        "no-mixed-type-exports"
    }

    fn check(&self, semantic: &Semantic<'_>, context: &mut RuleContext<'_>) {
        for node in semantic.nodes().iter() {
            let (span, kind, specifiers) = match node.kind() {
                AstKind::ExportNamedDeclaration(export) => {
                    (export.span, export.export_kind, &export.specifiers)
                }
                AstKind::ExportFromDeclaration(export) => {
                    (export.span, export.export_kind, &export.specifiers)
                }
                _ => continue,
            };
            if kind == ImportOrExportKind::Type {
                continue;
            }
            let has_type = specifiers
                .iter()
                .any(|item| item.export_kind == ImportOrExportKind::Type);
            let has_value = specifiers
                .iter()
                .any(|item| item.export_kind == ImportOrExportKind::Value);
            if has_type && has_value {
                context.report(
                    span,
                    "타입과 값을 하나의 export 문에 섞지 마세요. export type { ... }와 export { ... }로 분리하세요.",
                );
            }
        }
    }
}

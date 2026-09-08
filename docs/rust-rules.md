# Rust 규칙 작성

규칙은 `src/rules/`에 작성하고 CLI와 함께 컴파일합니다. YAML 및 런타임 Rust 스크립트는 지원하지 않습니다.

```rust
use ai_lint::rule::{Rule, RuleContext};
use oxc_ast::AstKind;
use oxc_semantic::Semantic;

pub struct NoDebugger;

impl Rule for NoDebugger {
    fn id(&self) -> &'static str { "no-debugger" }

    fn check(&self, semantic: &Semantic<'_>, context: &mut RuleContext<'_>) {
        for node in semantic.nodes().iter() {
            if let AstKind::DebuggerStatement(statement) = node.kind() {
                context.report(statement.span, "debugger를 제거하세요");
            }
        }
    }
}
```

저장소 내부에서는 `ai_lint::rule` 대신 `crate::rule`을 사용합니다.
파일을 추가한 뒤 `src/rules/mod.rs`에 모듈, `IDS` 항목, `select` 생성 분기를 등록하세요.
`ast.rs`는 호출 이름·바인딩·상위 함수·반복문 본문·소스 범위를 다루는 공통 도구입니다.
바인딩은 Oxc 심볼을 사용해 이름 가려짐과 블록 스코프를 구분합니다.
새 규칙에는 정상·위반·스코프 경계 사례의 Rust 테스트를 추가합니다.

```sh
cargo test --locked --offline
cargo build --release --locked
target/release/ai-lint rules
target/release/ai-lint check --rules no-alert --rules no-console-log src/App.tsx
```

`--rules ID`는 반복 가능하며 생략하면 `no-set-state-in-effect`만 실행합니다.
알 수 없는 ID, 중복 ID는 오류입니다. ID 대신 YAML 경로나 Rust 파일 경로를 넘길 수 없습니다.
Rust 호출부는 `RuleEngine::new(vec![Box::new(MyRule)])` 또는 `rules::select`를 사용합니다.

## AI 판단

`context.request_model_with_message(span, criteria, source, message)`로 요청을 수집합니다.
소스는 AST 순회가 끝난 뒤 설정한 서버로 전송됩니다. 후보가 없으면 요청하지 않습니다.
`pass`는 통과, `violation`은 진단, `unknown`·모델 미설정·통신 실패는 실행 오류입니다.
`prefer_functional_transforms.rs`에서 함수별 중복 요청 제거와 판단 기준을 볼 수 있습니다.
모델의 오류 처리·문맥 전달을 변경할 때는 모의 모델 테스트로 검증하세요.

기본 effect 규칙은 이름이 `useEffect`/`React.useEffect`인 인라인 콜백의
`setState` 또는 같은 바인딩의 useState setter를 찾습니다. import 출처와 값의 별칭은 추적하지 않습니다.
컬렉션 규칙은 지역 빈 배열에 반복문으로 직접 `.push`하는 함수를 후보로 선택합니다.
중첩 함수·클래스 경계를 구분하며 순수성, 부수 효과, 조기 종료 등은 AI가 판단합니다.
전체 실패를 부분 성공으로 바꾸는 것은 리팩터링이 아닌 별도 정책 변경입니다.

## Hook 이전

기존 `ruleFiles` 설정은 다음 명령으로 명시적으로 교체합니다. 이전 설정은 설치기가 백업합니다.

```sh
npm run hooks:install -- --global --agent claude-code \
  --rules no-set-state-in-effect --rules prefer-functional-transforms \
  --env-file /absolute/path/to/.env
```

새 설정의 `ruleIds`에는 규칙 ID를 저장합니다. 설치 시 실행 파일의 `rules` 목록으로 검증합니다.
기존 설정을 그대로 실행하면 재설치 안내와 함께 실패하며 다른 규칙으로 조용히 대체하지 않습니다.
규칙 코드·AI 판단 기준 수정에는 재빌드가 필요하고 `.env` 값 수정에는 필요 없습니다.

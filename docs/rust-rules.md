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
`src/rules/ast_helpers.rs`는 룰 내부에서 공유하는 AST 탐색 도구입니다.
생성된 AST에서 호출 이름·바인딩·상위 함수·반복문 본문·소스 범위를 확인하며,
같은 폴더의 룰은 `super::ast_helpers`로 가져옵니다. 소스 파싱은 `analyzer`가 담당합니다.
바인딩은 Oxc 심볼을 사용해 이름 가려짐과 블록 스코프를 구분합니다.
새 규칙에는 정상·위반·스코프 경계 사례의 Rust 테스트를 추가합니다.

## 파일 범위 지정

규칙이 검사할 파일은 `Rule::scope()`로 선언합니다. 엔진이 규칙을 실행하기 전에 거르므로
규칙 본문에 파일명 조건을 다시 쓰지 않습니다.

| `RuleScope` | 검사 대상 |
| --- | --- |
| `AllFiles` (기본값) | 모든 파일 |
| `TestFiles` | `*.test.ts`, `*.test.tsx` |
| `NonTestFiles` | 그 외 모든 파일 |

```rust
use crate::rule::{Rule, RuleContext, RuleScope};

impl Rule for NoDebugger {
    fn id(&self) -> &'static str { "no-debugger" }
    fn scope(&self) -> RuleScope { RuleScope::NonTestFiles }
    fn check(&self, semantic: &Semantic<'_>, context: &mut RuleContext<'_>) { /* ... */ }
}
```

테스트 판정은 파일명만 봅니다. `*.browser.test.tsx`처럼 접미사가 일치하면 포함하고,
`*.spec.ts`·JS 테스트·`__tests__` 디렉터리·`a.test.ts/b.ts`처럼 디렉터리 이름만 일치하는
경로는 포함하지 않습니다. 접미사를 넓히려면 `src/rule.rs`의 `is_test_file`을 수정합니다.

`RuleEngine::check(input)`은 `RuleInput.path`를 사용해 파일 범위를 판정합니다.
접미사보다 세밀한 파일별 정책이 필요하면 `context.file_path()`에서 경로를 직접 읽습니다.

```sh
cargo test --locked --offline
cargo build --release --locked
target/release/ai-lint rules
target/release/ai-lint check --rules no-alert --rules no-console-log src/App.tsx
```

`--rules ID`는 반복 가능하며 생략하면 `no-set-state-in-effect`만 실행합니다.
알 수 없는 ID, 중복 ID는 오류입니다. ID 대신 YAML 경로나 Rust 파일 경로를 넘길 수 없습니다.
Rust 호출부는 `RuleEngine::new(vec![Box::new(MyRule)])` 또는 `rules::select`를 사용합니다.

## 분석과 규칙 실행

`src/analyzer/`는 다른 프로젝트 모듈에 의존하지 않습니다. `ChangedFile`의 현재 소스를
파싱해 실제 Oxc AST, 문법 오류, 변경 메타데이터를 반환하며 규칙이나 모델을 실행하지 않습니다.
파일 선택과 분석·규칙 실행의 연결은 CLI 등 호출부가 담당합니다.

엔진은 자체 입력 타입인 `RuleInput`만 사용하며 `analyzer`의 타입을 참조하지 않습니다.
`src/rule_engine/`의 `mod.rs`는 엔진 실행, `types.rs`는 `RuleInput`·`RuleCheck`,
`tests.rs`는 엔진 테스트를 담당합니다. 공개 경로는 `ai_lint::rule_engine::{RuleEngine, RuleInput, RuleCheck}`입니다.
AST·경로·변경 메타데이터를 입력받으므로 다른 파서 호출부에서도 직접 사용할 수 있습니다.
`src/pipeline.rs`의 `From<&AnalyzedFile>` 구현이 두 모듈의 데이터 변환을 담당합니다.
분석기와 엔진은 각각 자신의 변경 메타데이터 타입을 소유하며, `RuleContext`도 엔진 쪽 타입만 사용합니다.
AST는 빌려서 전달하므로 변환 과정에서 복사하거나 다시 파싱하지 않습니다.

```rust
use ai_lint::{
    analyzer::{Allocator, Analyzer, ChangedFile},
    rule_engine::RuleInput,
    rules,
};

fn check_example() -> Result<(), Box<dyn std::error::Error>> {
    let file = ChangedFile::new("example.ts", "alert(2);")
        .with_previous_source("alert(1);");
    let engine = rules::select(&["no-alert".into()])?;
    let (syntax_errors, check) = {
        let allocator = Allocator::default();
        let analyzed = Analyzer::analyze(&allocator, &file)?;
        let input = RuleInput::from(&analyzed);
        let check = engine.check(input)?;
        (analyzed.syntax_errors, check)
    };
    let violations = engine.resolve(check)?;
    println!("syntax: {syntax_errors:?}, violations: {violations:?}");
    Ok(())
}
```

AST의 수명은 입력 파일과 allocator에 묶입니다. `check`는 AST를 다시 파싱하지 않고
공유 semantic AST를 구성합니다. 연결 코드는 문법 오류가 있거나 파서가 중단되면
`RuleInput.ast`를 `None`으로 전달하고, 엔진은 해당 입력의 규칙 실행을 건너뜁니다.
`RuleInput`을 직접 구성할 때도 성공적으로 파싱한 AST만 `Some`으로 전달해야 합니다.
`resolve`는 AST 없이도 동작하며 소유권이 독립적인 요청만 모델에 전달합니다.

변경 메타데이터는 분석 결과의 `analyzed.changes`와 규칙의 `context.changes()`에서 읽습니다.
각각 `analyzer::ChangeMetadata`와 `rule::ChangeMetadata`이며 연결 코드가 값을 보존해 변환합니다.

- `ChangedFile::new(path, source)`와 `ChangedFile::read(path)`는 비교할 이전 소스가 없으므로
  `ChangeKind::Unknown`을 반환합니다. 기존 CLI도 이 경로를 사용하며 Git 비교는 수행하지 않습니다.
- `ChangedFile::added(path, source)`는 `Added`, `with_previous_source(previous)`는
  내용 비교에 따라 `Modified` 또는 `Unchanged`를 반환합니다. 이전의 빈 파일과 새 파일을 구분합니다.
- `previous_bytes`·`current_bytes`는 소스의 바이트 길이입니다. 이전 소스가 없으면 `previous_bytes`는 `None`입니다.
- `range.before`·`range.after`는 모든 편집을 감싸는 하나의 UTF-8 바이트 범위입니다.
  끝 위치는 제외하고 문자 중간에서 자르지 않습니다. 삽입·삭제는 한쪽 범위가 비어 있습니다.
  여러 편집 사이의 변경되지 않은 코드도 포함할 수 있으며 개별 diff hunk 목록은 아닙니다.
  변경이 없거나 이전 소스를 모르면 `range`는 `None`입니다.

입력은 현재 존재하는 파일의 스냅샷입니다. 파일 삭제·이름 변경의 추적은 호출부의 책임입니다.
기존 규칙은 계속 파일 전체를 검사하며 변경 범위만으로 자동 필터링하지 않습니다.
새 정책에서 필요한 경우 각 규칙이 변경 메타데이터를 이용합니다.

## 와일드카드 export 금지

`no-wildcard-export`는 `export *`, `export type *`, `export * as name` 및
`export type * as name`을 금지하는 AST 전용 규칙입니다. 필요한 이름을
`export { Foo, bar } from './module'`처럼 명시하세요. 이름 있는 export,
default export, namespace import는 허용합니다. 실제 외부 사용 여부는
프로젝트 간 참조 분석을 수행하지 않으므로 이 규칙만으로 증명하지 않습니다.

## 금지된 React 호출

`no-create-context`와 `no-forward-ref`는 `ast_helpers::calls_module_export`를 공유하는 AST 전용
규칙입니다. 호출식만 검사하며 import 선언 자체나 타입 참조는 보고하지 않습니다.

판정은 이름 비교가 아니라 **바인딩 해석**입니다. 식별자를 `reference_id` → `symbol_id` →
선언 노드로 되짚어, 그 바인딩이 실제로 `react`의 해당 export인지 확인합니다.

- `ImportSpecifier` → 원래 export 이름과 모듈로 판정. `import { name as alias }`의 별칭도 잡습니다.
- `ImportDefaultSpecifier`·`ImportNamespaceSpecifier` → 모듈 전체 바인딩.
  `React.name(...)` 멤버 호출은 `React`가 이 둘 중 하나로 해석될 때만 보고합니다.
- `VariableDeclarator` → 초기화식을 따라 재귀합니다. `const alias = name`,
  `const { name } = React`, `const { name: alias } = React`를 모두 원래 export로 되돌립니다.
  순환을 막기 위해 12단계에서 멈춥니다.
- 선언을 찾을 수 없는 식별자만 이름으로 판정합니다. import 없이 붙여넣은 조각을
  웹 플레이그라운드에서 검사할 수 있게 하기 위한 예외입니다.

따라서 같은 이름이어도 다른 모듈의 export(`import { forwardRef } from '@shared/table'`),
무관한 객체의 메서드(`store.createContext()`), 지역 함수는 보고하지 않습니다.

### createContext 직접 사용 금지

`no-create-context`는 `createContext` 호출을 금지하고 프로젝트의 `createSafeContext`를
쓰도록 안내합니다.

```ts
// 위반
const ThemeContext = createContext<Theme | null>(null);

// 허용
const ThemeContext = createSafeContext<Theme>();
```

`createSafeContext` 구현부는 스스로 `createContext`를 호출해야 하므로 예외로 둡니다.
호출을 감싸는 상위 선언 이름이 `createSafeContext`이면 보고하지 않으며,
`function createSafeContext()`와 `const createSafeContext = () => ...` 두 형태를 인식합니다.

### forwardRef 사용 금지

`no-forward-ref`는 React 19에서 함수 컴포넌트가 `ref`를 일반 prop으로 받을 수 있게 되어
`forwardRef`가 deprecated된 것을 근거로 호출을 금지합니다. 예외는 없습니다.

```tsx
// 위반
const Input = forwardRef<HTMLInputElement, Props>((props, ref) => <input ref={ref} />);

// 허용
function Input({ ref, ...props }: Props) {
  return <input ref={ref} {...props} />;
}
```

`ForwardRefExoticComponent`·`ForwardedRef` 같은 타입 참조와 이미 작성된 `ref` prop의
타입 정합성은 검사하지 않습니다. 이는 TypeScript와 React 버전이 판단할 문제입니다.

### 공통 한계

모듈 이름은 `react`와 정확히 일치해야 합니다. `preact/compat`이나 사내 재수출 모듈도
막으려면 허용 모듈 목록으로 확장해야 합니다. `require('react').forwardRef()` 같은 CJS
접근은 심볼로 해석되지 않아 놓칩니다. `createSafeContext`의 존재 여부나 시그니처,
설치된 React 버전은 검증하지 않습니다.

## 데이터 요청 hook mock 금지

`no-query-hook-mocking`은 `RuleScope::TestFiles`를 선언한 AST 전용 규칙으로
`*.test.ts`와 `*.test.tsx`에만 적용합니다. `*.browser.test.tsx`도 포함하고,
일반 소스·`*.spec.ts`·JS 테스트에는 적용하지 않습니다.
AI 요청이나 다른 소스 파일의 탐색 없이 현재 테스트 파일만 분석합니다.

탐지하는 mock:

- Vitest/Jest의 `mock`, `doMock`, `unstable_mockModule` 모듈 대체 및 자동 mock.
  `vi.mock(import('...'), factory)`와 `{ spy: true }`도 포함합니다.
- `spyOn`·`replaceProperty`로 데이터 hook 대체.
- `mockReturnValue[Once]`, `mockImplementation[Once]`, `mockResolvedValue[Once]`,
  `mockRejectedValue[Once]`, `withImplementation`으로 hook mock 설정.
  import 별칭, `vi.mocked(useProjects)`를 저장한 지역 변수, TS 타입 단언을 따라갑니다.

데이터 hook은 `use` + 대문자로 시작하는 이름 중 다음 단서로 분류합니다.

- `Query`·`Queries`·`Mutation`·`Subscription`으로 끝나거나 `useSuspense`로 시작하는 이름,
  `useSWR`·`useSWRInfinite`·`useSWRMutation`·`useMutationState`.
- mock/import 모듈 경로에 `api` 세그먼트가 있는 hook. 예: `../../api/use-upload-log`.
- GEBRA의 `@entities/auth` 및 하위 경로(`@x/message` 등)에서 제공하는
  `useUser`·`useAuthSession`. 실제 구현이 사용자 쿼리를 사용하므로 반환값 형태와 무관하게
  데이터 hook으로 분류합니다. 같은 이름이어도 다른 모듈 출처는 이 단서에 포함하지 않습니다.
- mock 반환 객체가 `mutate`, `mutateAsync`, `refetch`, `fetchNextPage`를 가지거나,
  `data`와 `isLoading`/`isPending`/`isError`/`isSuccess`/`error`/`status`/`isFetching`을 함께 가짐.
  예: `useProjects`, `useBilling`처럼 이름에 Query가 없는 wrapper도 검사합니다.
- TanStack/React Query, Apollo Client, SWR의 알려진 모듈 전체 자동 mock.

```ts
// 위반: wrapper도 실제 hook을 우회합니다.
vi.mock('@entities/project', () => ({
  useProjects: vi.fn(() => ({ data: [], isLoading: false }))
}));

// 허용: 실제 hook과 provider를 사용하고 네트워크 응답을 제어합니다.
server.use(http.get('/projects', () => HttpResponse.json([])));
```

`vi.fn` 없는 일반 함수 대체도 검사합니다. 부분 mock에서 실제 hook을 그대로 보존하는
`useQuery: actual.useQuery`나, mock 동작을 설정하지 않는 `vi.mocked(useQuery)` 자체는 허용합니다.
라우팅·일반 UI 및 다른 인증 hook mock은 위 데이터 hook 단서가 없으면 보고하지 않습니다.
네트워크 API 함수 자체의 mock, store mock 금지는 이 규칙의 범위가 아닙니다.

단서는 파일 내 정적 휴리스틱이며 데이터 hook임을 타입/구현으로 증명하지 않습니다.
이름·경로·반환 형태 단서가 없는 커스텀 hook, 동적으로 계산한 이름, 외부 helper에 숨긴 mock,
직접 대입(`hooks.useQuery = stub`)은 놓칠 수 있습니다. 별칭은 순환 방지를 위해 12단계까지 추적합니다.
단순한 로컬 `data`/로딩 상태 hook도 같은 형태라면 진단될 수 있습니다.

```sh
target/release/ai-lint check --rules no-query-hook-mocking src/example.test.tsx
```

기본 규칙 선택은 바뀌지 않습니다. CLI의 `--rules` 또는 hook 설정의 `ruleIds`에 추가하세요.
별도 non-blocking severity는 없으며 기존 규칙과 같이 위반 시 CLI exit 1, Claude hook의
`decision: block` 및 MSW 안내를 반환합니다.

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

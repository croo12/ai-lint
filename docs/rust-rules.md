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
파일별 정책은 `context.file_path()`에서 경로를 읽습니다. `Analyzer`는 경로를 전달하며,
엔진을 직접 사용할 때는 `check_file(program, path)`를 사용하세요.
기존 `check(program)`은 경로가 없으므로 파일명에 제한된 규칙은 실행하지 않습니다.
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

## 와일드카드 export 금지

`no-wildcard-export`는 `export *`, `export type *`, `export * as name` 및
`export type * as name`을 금지하는 AST 전용 규칙입니다. 필요한 이름을
`export { Foo, bar } from './module'`처럼 명시하세요. 이름 있는 export,
default export, namespace import는 허용합니다. 실제 외부 사용 여부는
프로젝트 간 참조 분석을 수행하지 않으므로 이 규칙만으로 증명하지 않습니다.

## 데이터 요청 hook mock 금지

`no-query-hook-mocking`은 `*.test.ts`와 `*.test.tsx`에만 적용하는 AST 전용 규칙입니다.
`*.browser.test.tsx`도 포함하고, 일반 소스·`*.spec.ts`·JS 테스트에는 적용하지 않습니다.
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
라우팅·인증 상태·일반 UI hook mock은 위 데이터 hook 단서가 없으면 보고하지 않습니다.
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

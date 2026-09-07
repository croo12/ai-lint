# ai-lint

JavaScript/TypeScript 소스를 파싱하고 AST 규칙을 검사하는 Rust CLI입니다.

규칙 작성과 엔진 확장은 [YAML 전용 규칙 아키텍처 결정](docs/adr/0001-yaml-only-rules.md)을 따릅니다.

## Claude Code / Codex hook 어댑터

`packages/adapters`는 파일 수정 후와 종료 직전에 YAML 규칙을 실행하는
TypeScript hook 라이브러리입니다. 설치·연결 예제는
[어댑터 사용법](packages/adapters/README.md)에 있습니다.

```sh
npm run build:cli
npm run build:adapters
npm run test:adapters
```

빌드 후 현재 프로젝트에 hook을 자동 등록할 수 있습니다. 기존 설정은 보존·백업합니다.

```sh
npm run hooks:install -- --agent both --source-root apps/web/src
```

`--agent codex` 또는 `--agent claude-code`로 대상을 선택하고, `--workspace PATH`로
다른 프로젝트를 지정할 수 있습니다. `--dry-run`은 변경 예정 경로만 표시합니다.
설치 후 에이전트를 다시 열고 `/hooks`에서 등록 상태와 필요한 신뢰 승인을 확인하세요.

CLI는 어댑터용 JSON 출력도 지원합니다.

```sh
cargo run -- check --format json src/App.tsx
```

JSON에는 `schemaVersion`, `exitCode`, 파일별 `syntaxErrors`·`violations`, 실행 `errors`가
포함됩니다. 위반에는 UTF-8 바이트 범위와 1부터 시작하는 행·열(유니코드 문자 기준)이 있습니다.

## 프로젝트 구성과 웹 플레이그라운드

Rust CLI는 저장소 루트에서 Cargo로 관리하고, 웹 프로젝트는 npm workspaces의
`apps/web`에서 관리합니다. Rust CLI와 YAML 규칙을 웹에서도 그대로 사용합니다.

```text
ai-lint/
├── src/          Rust CLI·분석 엔진
├── rules/        공통 YAML 규칙
├── tests/        Rust 테스트
└── apps/web/     React + TypeScript 플레이그라운드
    ├── src/      코드 편집기·규칙 목록·결과 화면
    ├── rules/    웹 데모용 추가 YAML 규칙
    └── server/   로컬 CLI 실행 API
```

```sh
npm install
npm run dev
```

표시되는 로컬 주소를 열면 왼쪽에서 TypeScript/TSX 코드를 수정하고, 오른쪽에서
규칙을 선택한 뒤 검사할 수 있습니다. 예제 3개, YAML 원문 펼치기, 오류 위치 이동,
정상·문법 오류·서버 오류 표시를 지원합니다.

웹 서버는 요청 코드를 임시 파일에 저장하고 Rust CLI를 실행한 뒤 파일을 삭제합니다.
검사 대상 코드는 실행하지 않습니다. 현재 웹 규칙은 AST 검사만 제공하며 모델 서버를
호출하지 않습니다. 인증키는 브라우저에 전달하지 않습니다.
이 서버는 **로컬 시연용**입니다. Vite 개발/미리보기 서버의 API를 사용하므로 정적
`dist` 파일만 호스팅하면 검사 기능은 동작하지 않습니다.

```sh
npm run build:web    # TypeScript 검사 + 웹 프로덕션 빌드
npm run check:web    # 웹 소스를 ai-lint로 검사
npm run test:web     # 설치된 Chrome으로 실제 브라우저 테스트
npm run build:cli    # Rust 릴리스 빌드
npm run test:cli     # Rust 테스트
npm run preview --workspace @ai-lint/web  # 빌드한 웹 + 로컬 검사 API
```

웹 실행·미리보기 전에 Rust CLI를 자동 빌드합니다. 브라우저 테스트는 Chrome이 필요하며
Edge를 사용할 경우 `PLAYWRIGHT_CHANNEL=msedge` 환경 변수를 설정합니다.
현재 Node.js 24와 npm 11에서 검증했습니다. 웹 프로젝트 설정은
[Vite React·TypeScript 템플릿](https://github.com/vitejs/vite/tree/main/packages/create-vite/template-react-ts)을 참고했습니다.

```sh
cargo run -- check src/App.tsx
```

종료 코드는 정상 `0`, 문법 오류 또는 규칙 위반 `1`, 파일 읽기 등의 실행 오류 `2`입니다.
진단 위치의 `start..end`는 UTF-8 바이트 범위이며 끝 위치는 포함하지 않습니다.

## 모듈

- `analyzer`: 파싱 후 문법 오류가 없으면 규칙 실행기에 AST 전달
- `rule`: 진단과 모델 요청 수집용 내부 문맥, `RuleViolation`
- `rule_engine`: 등록된 규칙 실행 및 위반 결과 수집
- `model`: 환경 설정, 모델 클라이언트 인터페이스, OpenAI 호환 HTTP 클라이언트
- `yaml_rule`: YAML 규칙 로딩·검증 및 AST 조건 평가

새 규칙은 YAML로 작성해 `--rules`로 지정합니다. `yaml_rule`이 범용 AST 쿼리를 실행하고,
분석 결과에는 소유권이 독립적인 진단을 저장합니다.

## YAML 규칙

규칙은 YAML로만 정의합니다. Rust의 `Rule` 구현이나 규칙 전용 조건을 추가하지 않습니다.
문법과 캡처·탐색·바인딩 비교의 정확한 의미는 [YAML v2 가이드](docs/yaml-rules.md)에 있습니다.

```sh
cargo run -- check --rules rules/no-set-state-in-effect.yaml src/App.tsx
cargo run -- check --rules first.yaml --rules second.yaml src/App.tsx
```

`--rules`를 지정하면 나열한 YAML만 실행합니다. 생략하면
`rules/no-set-state-in-effect.yaml`을 빌드 시 포함한 기본 규칙을 실행합니다.
자동 디렉터리 탐색은 하지 않으며 기본 규칙 변경 반영에는 재빌드가 필요합니다.
중복 ID, 읽기 실패, 잘못된 YAML, 쿼리 실행 한도 초과는 실행 오류(종료 코드 `2`)입니다.

```yaml
version: 2
id: no-console-log
message: console.log를 사용하지 마세요.
match:
  kind: CallExpression
  properties:
    callee.type: MemberExpression
    callee.computed: false
    callee.object.type: Identifier
    callee.object.name: console
    callee.property.name: log
```

AST 노드 속성은 Oxc가 생성한 ESTree JSON의 경로로 접근합니다.
`at`, `child`, `descendant`, `ancestor`로 탐색하고,
`capture`와 `same_binding`으로 선언과 참조를 연결할 수 있습니다.
보고와 모델 요청은 최상위 `match` 노드당 한 번입니다.

`rules/examples/prefer-functional-transforms.yaml`은 지역 빈 배열 선언,
반복문, 같은 배열의 `push`를 YAML만으로 연결합니다.
중첩 함수 제외와 함수당 한 번 보고도 YAML의 선택·탐색 범위로 표현합니다.
후보만 AI에 전달하고, AI가 함수형 표현의 적합성과 동작 보존 여부를 판단합니다.
이 예제는 기본 활성화되지 않습니다.

기본 effect 규칙도 같은 범용 문법을 사용합니다.
`useEffect`/`React.useEffect`의 인라인 콜백에서 `setState` 호출 또는
`useState`/`React.useState`의 배열 구조 분해로 얻은 setter 호출을 찾습니다.
구조 분해 setter는 실제 바인딩으로 비교하므로 다른 스코프의 같은 이름과 구분됩니다.
중첩 함수 호출은 포함하며 import 별칭과 값의 간접 전달은 추적하지 않습니다.
함수 이름 자체의 판별은 YAML의 속성 비교이며 React import 출처를 증명하지 않습니다.

**마이그레이션:** v1과 `callee`, `callback`, `isStateSetter`,
작업 중이던 `hasLoopAccumulator`는 지원하지 않습니다.
저장소의 기본·예제·웹 규칙은 v2로 전환했습니다. 외부 규칙은
[마이그레이션 안내](docs/yaml-rules.md#v1에서-전환)를 따라 수정해야 합니다.

## 원격 모델 설정

`.env.example`을 `.env`로 복사한 뒤 값을 입력합니다. 실제 인증키가 들어가는
`.env`와 `.env.*`는 Git에서 제외하며 `.env.example`만 공유합니다.

```dotenv
AI_LINT_MODEL_BASE_URL=https://your-server.example/v1
AI_LINT_MODEL_NAME=your-model
AI_LINT_MODEL_API_KEY=your-key
AI_LINT_MODEL_TIMEOUT_SECS=30
AI_LINT_MODEL_JSON_MODE=false
```

- URL은 API 기본 경로입니다. `/chat/completions`를 자동으로 덧붙입니다.
  프록시 경로도 보존합니다. 예: `/proxy/v1` → `/proxy/v1/chat/completions`.
- 인증이 없는 서버라면 인증키를 비워둘 수 있습니다.
- 실행 디렉터리의 `.env`를 읽으며, 프로세스 환경 변수가 파일 값보다 우선합니다.
  부모 디렉터리의 `.env`는 자동 탐색하지 않습니다.
- 다른 파일은 `cargo run -- check --env-file config.env src/App.tsx`로 지정합니다.
- 파일이 없거나 URL·모델명·키가 모두 비어 있으면 모델을 설정하지 않습니다.
  일부 값만 채웠거나 값이 잘못되었다면 설정 오류로 종료합니다.
- 시간 제한은 요청당 1~3600초이며 기본값은 30초입니다.
- 서버가 `response_format: {"type":"json_object"}`를 지원할 때만
  `AI_LINT_MODEL_JSON_MODE=true`로 설정합니다. 기본값은 호환성을 위해 `false`이며,
  이 경우에도 프롬프트로 JSON을 요청하고 반환값을 엄격히 검증합니다.

클라이언트는 [OpenAI Chat Completions 형식](https://developers.openai.com/api/reference/resources/chat)의
비스트리밍 요청과 `choices[0].message.content` 응답을 사용합니다.
모델 응답 본문은 다음 형식이어야 합니다.

```json
{"decision":"violation","reason":"위반 이유"}
```

`decision`은 `violation`, `pass`, `unknown` 중 하나이며 `reason`은 비어 있지 않은
문자열이어야 합니다. 판단 불가, 설정 누락, HTTP 오류, 시간 초과, JSON 오류,
출력 잘림은 검사 실패(종료 코드 `2`)로 처리합니다. 위반은 `1`, 통과는 `0`입니다.
오류 출력에는 인증키나 서버의 원본 오류 응답을 포함하지 않습니다.

## 모델을 사용하는 규칙 추가

YAML에 `judge`를 추가합니다. 별도 Rust 규칙은 작성하지 않습니다.

```yaml
version: 2
id: review-function
message: 함수가 프로젝트 기준을 위반합니다.
match:
  kind: [FunctionDeclaration, FunctionExpression, ArrowFunctionExpression]
judge:
  context: matched_node
  criteria: |
    오류를 기록하지 않고 무시하는 catch 블록이 있으면 violation입니다.
    그렇지 않으면 pass, 판단에 필요한 문맥이 부족하면 unknown을 반환하세요.
```

`matched_node`(기본값), `enclosing_function`, `source_file` 문맥을 지원합니다.
선택한 소스 범위는 설정한 원격 모델 서버로 전송됩니다.
`violation`이면 YAML 메시지를 보고하고, `pass`면 통과합니다.
`unknown`·모델 미설정·통신 오류는 실행 오류입니다.
후보가 없으면 모델을 호출하지 않습니다.

Rust에서 호출할 때도 YAML을 로드합니다.

```rust
let rule = ai_lint::yaml_rule::YamlRule::load("rules/examples/contextual-effect.yaml")?;
let engine = ai_lint::rule_engine::RuleEngine::new(vec![rule]);
```

모델 클라이언트는 기존 `RuleEngine::with_model`로 주입합니다.
실행 예제 역시 YAML 규칙 파일을 받습니다.

```sh
cargo run --example model_rule -- src/App.tsx rules/examples/contextual-effect.yaml
cargo test --locked --offline
cargo fmt --check
cargo clippy --locked --offline --all-targets -- -D warnings
```

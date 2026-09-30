# ai-lint

JavaScript/TypeScript 소스를 파싱하고 AST 규칙을 검사하는 Rust CLI입니다.

규칙 작성과 엔진 확장은 [Rust 전용 규칙 아키텍처 결정](docs/adr/0002-rust-only-rules.md)을 따릅니다.

## Claude Code / Codex hook 어댑터

`packages/adapters`는 파일 수정 후 Rust 규칙을 실행하는
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

설치 시 `--rules`를 생략하면 신규 설치·재설치 모두 전체 룰을 적용합니다.
특정 룰만 적용하려면 `--rules ID`를 반복 지정하세요. AI 심사 룰에는 모델 설정이 필요합니다.

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

폴더는 역할을 기준으로 나눕니다. `apps/`에는 실행 앱, `packages/`에는 재사용 모듈을 두며,
예제와 테스트는 해당 앱·패키지 안에 둡니다. Rust 패키지는 `Cargo.toml`,
TypeScript 패키지는 `package.json`으로 관리합니다.

```text
ai-lint/
├── apps/
│   ├── cli/               Rust CLI·전체 실행 조합
│   │   ├── src/           main·pipeline·공개 라이브러리 경로
│   │   ├── tests/         CLI·crate 통합·의존성 경계 테스트
│   │   └── examples/      Rust 사용 예제
│   └── web/               React + TypeScript 플레이그라운드
├── packages/
│   ├── analyzer/          변경 파일 파싱·변경 메타데이터
│   ├── rule-contract/     공통 룰 계약 crate
│   ├── rule-engine/       규칙 실행·입출력 타입
│   ├── rules/             규칙 구현·AI 판단
│   └── adapters/          TypeScript hook 라이브러리
│       └── examples/      hook 연결·설정 예제
├── docs/                  작성 가이드·아키텍처 결정
├── scripts/               저장소 검증 스크립트
├── Cargo.toml             Rust workspace 설정
└── package.json           npm workspace 설정·공통 실행 명령
```

명령은 저장소 루트에서 실행합니다. Cargo와 npm은 각각 명시된 멤버만 관리하며,
Rust 빌드 결과는 루트 `target/`에 모입니다. 웹에서도 같은 Rust CLI와 규칙을 사용합니다.

```sh
npm install
npm run dev
```

표시되는 로컬 주소를 열면 왼쪽에서 TypeScript/TSX 코드를 수정하고, 오른쪽에서
규칙을 선택한 뒤 검사할 수 있습니다. 예제 3개, Rust 원문 펼치기, 오류 위치 이동,
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

## Crate 경계

Cargo workspace의 `apps/cli` 앱과 4개 라이브러리 crate로 구성합니다.

| Crate | 역할 | 프로젝트 내부 의존성 |
| --- | --- | --- |
| `ai-lint` | CLI, `pipeline`, 구성과 실행 순서 관리 | analyzer, rule-engine, rules |
| `ai-lint-analyzer` | AST·문법 오류·변경 메타데이터 반환 | 없음 |
| `ai-lint-rule-contract` | `Rule`, `RuleContext`, 변경 메타데이터, 진단·추가 판단 요청 타입 | 없음 |
| `ai-lint-rule-engine` | 입력 AST에 룰을 실행하고 결과 수집 | rule-contract |
| `ai-lint-rules` | 룰 구현·ID 선택·AI 판단·내부 AST 탐색 도구 | rule-contract |

각 crate는 선언된 의존성만 참조할 수 있습니다. `apps/cli/tests/architecture.rs`는 Cargo 의존성 선언에도
위 경계를 적용하며 개발·빌드 의존성도 검사합니다. 여러 crate를 사용하는 통합 테스트는 `apps/cli/tests/`에 둡니다.
`cargo test --locked`는 기본적으로 모든 workspace crate를 검사합니다.
결정 배경은 [crate 분리 ADR](docs/adr/0003-crate-boundaries.md)에 있습니다.

호출부가 `ChangedFile`과 `Allocator`를 소유하고 `Analyzer::analyze`의 반환값을
`RuleInput`으로 변환해 `RuleEngine::check`에 전달합니다. 두 모듈은 서로의 타입을 참조하지 않으며
변환은 `pipeline::rule_input` 함수가 담당합니다. `rules::select`는 룰 목록을 반환하고
호출부가 `RuleEngine::new`로 엔진을 생성합니다. AST는 입력 소스와 allocator를 빌리며,
`RuleCheck`의 위반 결과와 추가 판단 요청은 독립적인 소유권을 가집니다. CLI는 AST를 해제한 뒤
`rules::ai_model::evaluate_reviews`로 추가 판단 요청을 처리하고 결과를 합칩니다. 모델 클라이언트는 CLI가
별도로 소유합니다. 사용 예와 변경 메타데이터는 [규칙 작성 가이드](docs/rust-rules.md#분석과-규칙-실행)에 있습니다.

## Rust 규칙

규칙은 `packages/rules/src/`에서 `Rule` 트레이트로 작성합니다.
[규칙 작성 가이드](docs/rust-rules.md)에 새 규칙 추가와 hook 이전 방법을 정리했습니다.

```sh
cargo run -- rules
cargo run -- check --rules no-alert --rules no-console-log src/App.tsx
```

규칙 ID: `no-set-state-in-effect`, `no-console-log`, `no-alert`,
`contextual-effect`, `prefer-functional-transforms`, `no-wildcard-export`,
`no-query-hook-mocking`, `no-useless-comments`, `no-unsafe-type-assertions`,
`no-create-context`, `no-forward-ref`.
`--rules`를 생략하면 기본 effect 규칙만 실행합니다.
규칙마다 `Rule::scope()`로 검사 대상 파일을 `AllFiles`·`TestFiles`·`NonTestFiles` 중에서
선언합니다. [파일 범위 지정](docs/rust-rules.md#파일-범위-지정)을 참고하세요.
`contextual-effect`와 `prefer-functional-transforms`는 후보에 대해서만 AI 판단을 요청합니다.
`no-useless-comments`는 파서가 추출한 모든 주석에 AI 판단을 요청합니다.
`no-wildcard-export`는 `export *` 대신 필요한 이름을 명시하도록 검사하는 AST 규칙입니다.
`no-query-hook-mocking`은 `*.test.ts`/`*.test.tsx`에서 데이터 요청 hook을 직접 mock하면
MSW 사용을 안내합니다. [탐지 범위와 예외](docs/rust-rules.md#데이터-요청-hook-mock-금지)를 참고하세요.
`no-useless-comments`는 주석을 기본 금지합니다. TODO·이슈 링크·JSDoc·타입 단언 설명도
자동 허용하지 않습니다. 코드·타입·검증·테스트로 대체하거나 이슈·ADR로 옮길 수 있으면 위반입니다.
필수 도구 지시문, 보존 의무가 있는 법적 고지, 대체 불가능한 외부 제약·안전 조건만
문맥상 필요성이 입증되고 최소한으로 작성된 경우 AI 심사를 거쳐 허용합니다.
주석마다 전체 파일 소스를 설정된 모델로 전달합니다. 주석이 없으면 모델을 호출하지 않으며,
모델 미설정·통신 실패·판단 불가는 통과가 아닌 실행 오류입니다.
`no-create-context`는 `createContext` 직접 호출을 금지하고 `createSafeContext` 사용을,
`no-forward-ref`는 deprecated된 `forwardRef` 대신 `ref` prop 사용을 안내합니다.
[탐지 범위와 예외](docs/rust-rules.md#금지된-react-호출)를 참고하세요.

YAML 규칙 엔진은 제거했습니다. 기존 `--rules FILE.yaml`은 `--rules ID`로,
hook의 `ruleFiles`는 `ruleIds`로 전환해야 합니다.
규칙 코드와 AI 판단 기준을 수정하면 재빌드가 필요합니다.

## 원격 모델 설정

`.env.example`을 `.env`로 복사한 뒤 값을 입력합니다. 실제 인증키가 들어가는
`.env`와 `.env.*`는 Git에서 제외하며 `.env.example`만 공유합니다.

```dotenv
AI_LINT_MODEL_BASE_URL=https://your-server.example/v1
AI_LINT_MODEL_NAME=Qwen/Qwen3.6-35B-A3B
AI_LINT_MODEL_API_KEY=your-key
AI_LINT_MODEL_TIMEOUT_SECS=30
AI_LINT_MODEL_RESPONSE_FORMAT=json_schema
```

- URL은 API 기본 경로입니다. `/chat/completions`를 자동으로 덧붙입니다.
  프록시 경로도 보존합니다. 예: `/proxy/v1` → `/proxy/v1/chat/completions`.
- 인증이 없는 서버라면 인증키를 비워둘 수 있습니다.
- 모델명은 서버에 등록한 ID 또는 별칭을 사용합니다.
- 기본 모델 설정은 Rust 빌드 시 저장소 루트의 `.env`에서 읽어 실행 파일에 포함합니다.
  실행 디렉터리의 `.env`와 런타임 환경 변수는 기본 설정에 영향을 주지 않습니다.
  `.env`를 변경하면 재빌드해야 하며 Cargo가 파일 변경을 감지합니다. 인증키도 빌드 산출물에 포함됩니다.
- 명시적인 런타임 재정의는 `cargo run -- check --env-file config.env src/App.tsx`로 지정합니다.
  이 옵션을 사용한 경우에만 해당 파일을 읽고 프로세스 환경 변수를 우선 적용합니다.
- 파일이 없거나 URL·모델명·키가 모두 비어 있으면 모델을 설정하지 않습니다.
  일부 값만 채웠거나 값이 잘못되었다면 설정 오류로 종료합니다.
- 시간 제한은 요청당 1~3600초이며 기본값은 30초입니다.
- `AI_LINT_MODEL_RESPONSE_FORMAT`은 기본값 `json_schema`로,
  `response_format.type=json_schema`와 `strict=true`를 전송합니다.
  스키마는 `decision`의 세 가지 값, 필수 `reason` 문자열(`minLength: 1`), 추가 필드 금지를 정의합니다.
  모델을 제공하는 추론 서버가 JSON Schema 제약 생성을 지원해야 합니다.
- 호환 모드는 `json_object`(JSON 객체만 강제) 또는 `text`(`response_format` 생략)로
  명시할 수 있습니다. 서버가 스키마 요청을 거부해도 자동으로 제약을 낮춰 재시도하지 않습니다.
- 기존 `AI_LINT_MODEL_JSON_MODE=true/false`는 새 설정이 없을 때 각각
  `json_object`/`text`로 동작합니다. 기존 `.env`에는
  `AI_LINT_MODEL_RESPONSE_FORMAT=json_schema`를 추가하면 스키마 모드가 우선합니다.
- 모든 모드에서 반환값을 엄격히 검증하며 공백뿐인 `reason`도 거부합니다.
  스키마 제약은 출력 형식을 제한하며 판단의 정확성을 보장하지는 않습니다.

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

Rust 규칙에서 `RuleContext::request_review_with_message`로 후보와 판단 기준을 전달합니다.
선택한 소스가 설정한 원격 서버로 전송되며, 후보가 없으면 모델 요청도 없습니다.
`violation`은 진단, `pass`는 통과, `unknown`·모델 미설정·통신 오류는 실행 오류입니다.
기본 `.env`는 빌드 시 읽으므로 값 변경 시 재빌드해야 합니다.

```sh
cargo run --example model_rule -- src/App.tsx contextual-effect
cargo test --locked --offline
cargo fmt --check
cargo clippy --locked --offline --all-targets -- -D warnings
```

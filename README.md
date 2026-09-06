# ai-lint

JavaScript/TypeScript 소스를 파싱하고 AST 규칙을 검사하는 Rust CLI입니다.

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
- `rule`: `Rule` 인터페이스, `RuleContext`, `RuleViolation`
- `rule_engine`: 등록된 규칙 실행 및 위반 결과 수집
- `model`: 환경 설정, 모델 클라이언트 인터페이스, OpenAI 호환 HTTP 클라이언트
- `yaml_rule`: YAML 규칙 로딩·검증 및 AST 조건 평가

새 규칙은 YAML로 작성해 `--rules`로 지정합니다. `yaml_rule`이 AST를 순회하고,
분석 결과에는 소유권이 독립적인 진단을 저장합니다.

## YAML 규칙

사용자 규칙은 YAML 파일 하나에 규칙 하나를 작성합니다.

```sh
cargo run -- check --rules rules/no-set-state-in-effect.yaml src/App.tsx
cargo run -- check --rules first.yaml --rules second.yaml src/App.tsx
```

`--rules`를 지정하면 나열한 YAML 규칙만 실행합니다. 생략하면
`rules/no-set-state-in-effect.yaml`을 빌드 시 실행 파일에 포함한 기본 규칙을 실행합니다.
따라서 기본 규칙은 실행 디렉터리에 의존하지 않으며 변경 반영에는 재빌드가 필요합니다.
자동 디렉터리 탐색은 하지 않습니다. 중복 ID, 읽기 실패, 잘못된 YAML,
지원하지 않는 필드·노드 종류·조건은 오류(종료 코드 `2`)로 처리합니다.

```yaml
version: 1
id: no-set-state-in-effect
message: useEffect에 setState를 넣어서는 안됩니다
match:
  kind: CallExpression
  callee: [useEffect, React.useEffect]
where:
  callback:
    index: 0
    contains:
      kind: CallExpression
      isStateSetter: true
```

이 규칙은 조건을 만족하는 **useEffect 호출당 한 건**을 보고합니다. 진단 위치도
useEffect 호출 전체입니다. 기본 검사도 같은 방식으로 보고합니다.

### 버전 1 문법

`id`, `message`, `match`는 필수이며 `version` 기본값은 `1`입니다.
ID에는 영문·숫자·`-`·`_`·`/`를 사용할 수 있습니다.

| 항목 | 의미 |
|---|---|
| `match.kind` | 현재 `CallExpression` 지원 |
| `match.callee` | 정확한 함수 이름 또는 이름 목록. `React.useEffect` 같은 점 표기 지원. 생략하면 모든 호출 |
| `match.isStateSetter` | setter 여부를 `true` 또는 `false`로 제한 |
| `where.callback` | `index`(0부터 시작) 위치의 인라인 함수·화살표 콜백에서 `contains` 선택자에 맞는 호출 검색 |
| `where.contains` | 현재 호출 자신을 제외한 하위 트리에서 선택자에 맞는 호출 검색 |
| `where.all` / `where.any` | 조건 목록을 모두/하나 이상 만족해야 함 |
| `where.not` | 하나의 조건을 부정 |

선택자의 필드는 모두 AND로 결합됩니다. 각 조건에는 `callback`, `contains`, `all`,
`any`, `not` 중 정확히 하나만 사용합니다. 논리 조건은 중첩할 수 있습니다.

```yaml
where:
  all:
    - callback:
        index: 0
        contains: {kind: CallExpression, isStateSetter: true}
    - not:
        contains: {kind: CallExpression, callee: subscribe}
```

`isStateSetter`는 `setState` 및 같은 파일의
`useState`/`React.useState` 구조 분해로 얻은 setter 이름을 검사합니다.
import 별칭, 변수 가려짐, 변수로 전달된 콜백, 간접 호출은 추적하지 않습니다.
계산된 프로퍼티 호출(`React['useEffect']`)은 `callee` 점 표기로 매칭하지 않습니다.
중첩 함수 내부도 검색하지만 문자열·주석은 호출로 취급하지 않습니다.
규칙 파일은 최대 256 KiB, 조건 중첩은 최대 32단계입니다.

### YAML에서 모델 판단 요청

`judge`를 추가하면 AST 조건을 만족한 후보만 모델에 전달합니다.
모델 설정이 없더라도 후보가 없으면 모델 요청 없이 완료합니다.

```yaml
judge:
  context: enclosing_function
  criteria: |
    외부 구독으로 받은 값을 반영하는 상태 업데이트는 허용합니다.
    그 외에는 위반입니다. 문맥이 부족하면 unknown을 반환하세요.
```

`context`는 `matched_node`(기본값: 매칭된 호출), `enclosing_function`(가장 가까운
상위 함수, 없으면 매칭된 호출), `source_file`(파일 전체)을 지원합니다.
선택한 범위의 소스가 설정한 원격 모델 서버로 전송됩니다.
`violation`이면 YAML의 `message`를 보고하고, `pass`면 보고하지 않습니다.
`unknown`이나 모델 오류는 검사 실패로 처리합니다.

전체 예제: `rules/examples/contextual-effect.yaml`.

## no-set-state-in-effect

`useEffect` 또는 `React.useEffect`의 인라인 화살표/함수 콜백 안에서
`setState(...)` 또는 같은 파일의 `useState`/`React.useState` 배열 구조 분해로
선언된 setter 호출을 발견하면 다음 문구를 출력합니다.

> useEffect에 setState를 넣어서는 안됩니다

콜백 내부에 중첩된 함수의 호출도 포함합니다. 문자열, 주석, setter 참조만 있는
표현식은 위반이 아닙니다. 의존성 배열 자체는 해당 effect의 콜백으로 검사하지 않습니다.

첫 버전은 이름을 기반으로 한 문법 검사입니다. import 별칭, 동일 이름의 다른 바인딩
(shadowing), 변수로 전달한 콜백, 별도 함수 내부의 간접 호출은 추적하지 않습니다.
따라서 동일 이름을 재사용하면 오탐이 발생할 수 있습니다.

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

규칙의 `check()`에서 `context.request_model(span, criteria, source_excerpt)`를 호출합니다.
`span`은 원본 파일의 진단 범위, `criteria`는 검사 기준, `source_excerpt`는 판단에 필요한
코드 조각입니다. 이 시점에는 네트워크 요청 없이 소유권이 독립적인 요청을 수집합니다.
분석기가 AST를 해제한 뒤 `RuleEngine::resolve()`가 요청을 순차 실행합니다.

라이브러리 사용 시 `ModelConfig::load()`와 `OpenAiCompatibleClient::new()`로 클라이언트를
만들고 `RuleEngine::with_model()`로 주입합니다. `Analyzer::analyze_file_with_rules()` 또는
`analyze_source_with_rules()`에 실행기를 전달하면 됩니다. `ModelClient`를 구현하여
다른 서버 구현이나 테스트용 클라이언트로 교체할 수도 있습니다.

현재 기본 `useEffect` 규칙은 AST 검사만 수행하므로 모델 서버를 호출하지 않습니다.
새 모델 규칙은 YAML의 `judge` 항목으로 정의하고 `--rules`로 지정합니다.

설정을 채운 후 다음 예제로 실제 연동을 시험할 수 있습니다. 이 예제는 **입력 파일 전체를
설정한 서버로 전송**하고, 명령행에서 받은 검사 기준으로 판단을 요청합니다.

```sh
cargo run --example model_rule -- src/App.tsx "오류를 기록하지 않고 무시하는 catch 블록이 있으면 위반입니다."
```

실제 서버의 모델 및 확장 기능 호환성은 설정을 입력한 뒤 확인해야 합니다.
자동 테스트는 외부 모델 대신 루프백 HTTP 모의 서버를 사용합니다.

```sh
cargo test --locked --offline
cargo fmt --check
cargo clippy --locked --offline --all-targets -- -D warnings
```

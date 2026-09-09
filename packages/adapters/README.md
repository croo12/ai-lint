# @ai-lint/adapters

Claude Code와 Codex의 동기 command hook에서 Rust 규칙을 실행하는 TypeScript 라이브러리입니다.
모델이 검사 도구를 선택할 필요 없이 hook 이벤트로 트리거됩니다. MCP 서버는 사용하지 않습니다.

## 준비

ai-lint 저장소 루트에서 실행합니다. Node.js 22 이상과 Rust 도구 체인이 필요합니다.

```sh
npm install
npm run build:cli
npm run build:adapters
```

이 패키지는 현재 저장소 내 workspace 패키지이며 npm에 게시되지 않았습니다.
다른 프로젝트에서는 빌드한 `dist/cli.js`와 Rust 실행 파일의 절대 경로를 사용하세요.

## 자동 설치

Claude Code 사용자 전역 설치는 다음 명령을 사용합니다.

```sh
npm run hooks:install -- --global --agent claude-code
```

사용자 홈의 `.claude/settings.json`에 병합하고 `.claude/ai-lint/adapter.json`을 생성합니다.
백업은 `.claude/ai-lint/backups/`에 보관합니다. 각 hook 입력의 `cwd`를 검사 루트로 사용하며
기본 Rust 규칙으로 해당 디렉터리의 JS/TS를 검사합니다. 전역 설치에는 `--workspace`와
`--source-root`를 함께 지정할 수 없습니다. 이 저장소의 빌드 결과를 참조하므로 이동·삭제하지 마세요.
이미 프로젝트 hook이 설치되어 있으면 전역 hook과 함께 실행될 수 있습니다.

빌드 후 ai-lint 저장소 루트에서 실행하면 현재 프로젝트에 두 에이전트의 hook을 등록합니다.

```sh
npm run hooks:install -- --agent both --source-root apps/web/src
```

`--agent claude-code` 또는 `--agent codex`로 하나만 선택할 수 있습니다.
다른 프로젝트에는 검사할 프로젝트 경로를 지정합니다.

```sh
npm run hooks:install -- --agent codex --workspace C:/projects/my-app --source-root src
```

- `.ai-lint/adapter.json`과 선택한 에이전트의 `.claude/settings.json`, `.codex/hooks.json`을 생성·병합합니다.
- 기존 설정과 다른 hook은 보존합니다. 재실행하면 관리 중인 hook을 중복 없이 갱신합니다.
- 변경되는 기존 파일은 `.ai-lint/backups/`에 원문 그대로 백업합니다. 필요하면 해당 파일을 원래 경로에 복사해 복구하세요.
  다른 프로젝트에서도 이 백업 디렉터리를 `.gitignore`에 추가하는 것을 권장합니다.
- `--dry-run`을 붙이면 파일을 쓰지 않고 변경 예정 경로만 출력합니다.
- `--rules ID`과 `--source-root PATH`는 여러 번 지정할 수 있습니다. `--env-file FILE`,
  `--bin PATH`, `--timeout-ms NUMBER`, `--hook-timeout SECONDS`도 지원합니다.
- 새 설정의 검사 범위 기본값은 프로젝트 전체입니다. 기존 어댑터 설정은 명시한 옵션만 갱신합니다.
  규칙을 생략하면 기본 Rust 규칙을 사용합니다. 실행 파일 기본값은 이 저장소의 릴리스 빌드입니다.
- 기존 JSON 설정이 잘못되었거나 검사 경로·실행 파일이 없으면 설정을 변경하지 않고 실패합니다.

설치 후 에이전트를 다시 열고 `/hooks`에서 등록 상태를 확인하세요.
Codex의 hook 신뢰 승인은 사용자가 직접 해야 하며 설치 명령이 우회하지 않습니다.
설치되는 명령은 이 저장소의 절대 경로를 참조하므로 저장소를 이동하면 재설치하세요.

아래는 자동 설치 대신 직접 연결할 때 사용하는 설정입니다.

## 1. 검사 범위 설정

`examples/adapter.config.json`을 원하는 위치에 복사해 수정합니다.

```json
{
  "workspace": "C:/projects/my-app",
  "binary": "C:/tools/ai-lint/target/release/ai-lint.exe",
  "sourceRoots": ["src", "apps/web/src"],
  "ruleIds": ["no-set-state-in-effect"],
  "envFile": ".env",
  "timeoutMs": 60000
}
```

Linux/macOS에서는 실행 파일의 `.exe`를 빼고 해당 시스템 경로를 사용합니다.
상대 `workspace`는 설정 파일 위치 기준, 나머지 경로는 workspace 기준입니다.
`sourceRoots`에는 존재하는 소스 디렉터리나 파일을 지정합니다. 기본값은 `["."]`이며,
의도적으로 오류가 있는 테스트 fixture는 검사 범위에서 제외하는 것이 좋습니다.
`ruleIds`를 생략하거나 비우면 실행 파일에 포함한 기본 Rust 규칙을 사용합니다.

모델 인증키는 기존 `.env`와 `AI_LINT_MODEL_*` 환경 변수를 사용합니다.
모델을 사용하는 규칙을 등록했다면 해당 코드 범위가 설정한 원격 서버로 전송됩니다.
`timeoutMs`는 CLI 호출 한 번의 제한이며, 많은 파일은 100개씩 나누어 검사합니다.
hook의 `timeout`은 전체 실행 제한이므로 모델 호출과 파일 수에 맞게 늘리세요.

## 2. Claude Code에 연결

`examples/claude.settings.json`의 경로를 바꾸어 프로젝트의 `.claude/settings.json`에
`hooks` 항목을 병합합니다. 기존 hook 목록을 덮어쓰지 마세요.
`PostToolUse`는 `Write`, `Edit`, `MultiEdit`, `Bash` 뒤에 실행하며 `Stop`도 등록합니다.

설정 위치와 출력 형식은 [Claude Code hooks 공식 문서](https://code.claude.com/docs/en/hooks)를
기준으로 작성했습니다. hook을 등록한 뒤 Claude Code에서 활성화 여부를 확인하세요.

## 3. Codex에 연결

`examples/codex.hooks.json`의 경로를 바꾸어 프로젝트의 `.codex/hooks.json`에 병합합니다.
`PostToolUse`에서 패치 및 셸 도구를, `Stop`에서 종료 직전 검사를 실행합니다.

[Codex hooks 공식 문서](https://learn.chatgpt.com/docs/hooks)에 따라 프로젝트 설정 계층과
hook에 대한 신뢰 검토가 필요합니다. lifecycle hooks를 지원하는 Codex 버전을 사용하세요.
설정이 비활성화되거나 신뢰되지 않았다면 어댑터가 자동 실행을 강제할 수 없습니다.

## 동작과 한계

- 파일 경로가 있는 `Write/Edit` 이벤트는 해당 파일을 검사합니다.
- `Stop`은 Git의 staged·unstaged 변경과 untracked 파일만 검사합니다.
  삭제된 파일, 검사 범위 밖의 파일, 변경 없이 이미 커밋된 파일은 제외합니다.
  파일명은 NUL 구분으로 처리해 공백·개행을 지원하며, rename 대상도 검사합니다.
  새 파일은 `.gitignore`를 따릅니다. 검사 대상 파일의 현재 전체 내용을 읽으며
  index에 저장된 내용이나 diff의 변경 행만 검사하는 것은 아닙니다.
  변경 파일이 없으면 검사기를 실행하지 않습니다. Git 저장소가 아니면 안내 후
  Stop 검사를 건너뛰고, Git 실행 실패는 오류로 보고합니다.
- 셸·패치 `PostToolUse` 이벤트는 기존처럼 `sourceRoots`의 JS/TS 파일을 재귀 탐색합니다.
  Git 저장소가 아니어도 동작하며 이미 커밋한 코드도 검사합니다.
- `node_modules`, `.git`, `target`, `dist`, `build`, `.next`, `coverage`,
  `test-results`, `playwright-report` 디렉터리는 탐색에서 제외합니다.
  재귀 탐색은 `.gitignore` 패턴을 해석하지 않으며 디렉터리 심볼릭 링크를 따라가지 않습니다.
- 소스 수정·명령 실행은 수행하지 않으며 원본 파일은 읽기만 합니다.
  지정한 실행 파일을 shell 없이 호출하고 JSON 결과를 검증합니다.
- 정상 결과는 `{}`, 규칙 위반·문법 오류·검사 실패는
  `{"decision":"block","reason":"파일:행:열 ..."}`을 stdout으로 반환하고 exit `0`으로
  종료합니다. 두 호스트가 JSON 결정을 읽도록 하기 위한 동작입니다.
- `PostToolUse`는 이미 발생한 수정 자체를 취소하지 않습니다. 에이전트에 오류를 전달합니다.
  `Stop`의 block 응답은 수정 작업을 계속하도록 요청합니다.
- `stop_hook_active: true`인 반복 Stop에서도 재검사하지만, 위반이 남으면
  `systemMessage`로 알리고 종료 차단은 반복하지 않습니다. 무한 루프를 막기 위한 정책이며,
  위반이 전혀 없는 상태에서만 종료한다는 절대적 보장은 제공하지 않습니다.
- 실패한 소스 탐색이나 CLI 실행을 정상 검사 결과로 처리하지 않습니다.
- 현재 자동 테스트는 실제 CLI와 모의 hook 입력을 검증합니다. 로그인한 Claude Code/Codex
  세션 내부의 hook 실행은 사용자가 등록한 뒤 확인해야 합니다.

## 프로그래밍 API

```ts
import { AiLintAdapter, handleHook, claudeCodeHooks, codexHooks } from '@ai-lint/adapters';

const report = await new AiLintAdapter({
  workspace: '/projects/my-app',
  binary: '/tools/ai-lint/target/release/ai-lint',
}).check(['src/App.tsx']);

const claudeSettings = claudeCodeHooks('/tools/ai-lint/packages/adapters/dist/cli.js', '/projects/my-app/adapter.config.json');
const codexSettings = codexHooks('/tools/ai-lint/packages/adapters/dist/cli.js', '/projects/my-app/adapter.config.json');
```

두 설정 생성 함수는 JSON 객체만 반환하며 파일을 쓰지 않습니다. 다음 명령도 설정을
stdout으로 출력할 뿐 자동 등록하지 않습니다.

```sh
node packages/adapters/examples/generate-config.mjs codex /absolute/dist/cli.js /absolute/adapter.config.json
npm run test:adapters
```

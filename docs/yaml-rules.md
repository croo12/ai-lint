# YAML 규칙 v2

규칙 작성의 유일한 인터페이스는 YAML입니다. Rust는 아래 범용 연산을 실행하며,
effect·컬렉션 변환 같은 정책을 알지 못합니다. AST는 Oxc 0.148의 TypeScript 필드를
포함한 ESTree JSON을 사용합니다. 새 노드 종류나 AST 속성을 쓰는 데 Rust 변경은 필요 없습니다.
단순 괄호는 파서에서 제거하므로 `(callback)`과 `callback`의 경로는 같습니다.

```yaml
version: 2
id: no-debugger
message: debugger를 제거하세요.
match:
  kind: DebuggerStatement
```

`version`, `id`, `message`, `match`는 필수입니다. `where`와 `judge`는 선택입니다.
`match`는 파일의 모든 AST 노드에 적용됩니다. `where`는 매칭한 같은 노드에서 이어서
평가하며 `match`의 캡처를 사용할 수 있습니다. 조건을 만족하는 조합이 여러 개라도
최상위 매칭 노드당 진단 또는 모델 요청은 한 번입니다.

## 속성 검사와 경로

| 연산 | 의미 |
|---|---|
| `kind` | ESTree `type` 문자열 하나 또는 OR 목록. 생략 가능 |
| `properties` | 경로별 JSON 값의 정확한 동등 비교. 모든 항목 AND |
| `exists` | 경로가 존재해야 함. 값이 `null`이어도 존재로 취급 |
| `capture` | 현재 AST 노드를 이름으로 저장 |
| `same_binding` | 두 경로의 Identifier가 같은 해석된 변수 바인딩인지 비교 |
| `before` | 왼쪽 AST 노드의 끝이 오른쪽 노드의 시작 이전 또는 같은 위치인지 비교 |

경로는 `callee.object.name`, `arguments.0`, `id.elements.1`처럼 점으로 구분합니다.
`.`은 현재 노드이며 `$array.id`는 `array` 캡처의 `id`, `$array`는 캡처 노드 자체입니다.
배열은 숫자 인덱스로 접근합니다. 빈 배열 검사는 `init.elements: []`처럼 작성합니다.
없는 경로는 매칭되지 않습니다. `properties`에서 문자열·숫자·불리언·null·배열·객체를
정확히 비교하며, 정규식이나 문자열 접두사로 자동 해석하지 않습니다.

```yaml
match:
  kind: VariableDeclarator
  capture: array
  properties:
    id.type: Identifier
    init.type: ArrayExpression
    init.elements: []
```

AST 이름은 ESTree 기준입니다. 함수는 `FunctionDeclaration`, `FunctionExpression`,
`ArrowFunctionExpression`이며 Oxc 내부 이름인 `Function`을 쓰지 않습니다.
속성 접근은 `MemberExpression`이고 점 접근을 제한하려면 `computed: false`를 검사합니다.
등록되지 않은 종류 이름이나 존재하지 않는 속성은 불일치로 처리하므로 오타에 주의하세요.
규칙 스키마 자체의 알 수 없는 연산 이름은 로드 오류입니다.

## 탐색과 논리 조합

| 연산 | 의미 |
|---|---|
| `at: {path, match}` | 경로로 지정한 AST 노드에서 쿼리 평가. 배열·스칼라는 대상이 아님 |
| `child: {match}` | 직접 자식 AST 노드 중 일치하는 노드 탐색 |
| `descendant: {match, stop_at?}` | 자신을 제외한 하위 AST 노드 탐색 |
| `ancestor: {match, stop_at?}` | 자신을 제외한 상위 AST 노드 탐색 |
| `all: [...]` | 순서대로 모두 만족. 앞 단계의 캡처를 뒤에서 사용 |
| `any: [...]` | 하나 이상 만족. 각 분기는 독립적으로 평가 |
| `not: {...}` | 내부 쿼리의 매칭이 없어야 함. 내부 캡처는 밖으로 전달하지 않음 |

하나의 쿼리에 종류·속성·캡처·비교 조건과 탐색/논리 연산 하나를 함께 사용할 수 있습니다.
여러 탐색을 결합할 때는 `all` 또는 `any`를 사용합니다. 직접 자식의 의미는 AST 노드
사이의 관계이며, `body` 같은 배열 컨테이너 자체는 노드가 아닙니다.
`stop_at`은 종류 하나 또는 목록으로, 경계 노드 자체와 그 너머를 모두 제외합니다.
시작 노드에는 적용하지 않습니다. `child`에는 `stop_at`을 지정할 수 없습니다.

```yaml
match:
  kind: [FunctionDeclaration, FunctionExpression, ArrowFunctionExpression]
where:
  at:
    path: body
    match:
      descendant:
        stop_at: [FunctionDeclaration, FunctionExpression, ArrowFunctionExpression]
        match:
          kind: DebuggerStatement
```

캡처는 한 평가 경로에서 이름을 중복 정의할 수 없습니다. `any` 뒤에서는 모든 분기가
정의한 캡처만 참조할 수 있습니다. 정의되지 않은 캡처는 로드 오류입니다.
탐색 결과의 첫 후보가 뒤 조건을 만족하지 않아도 다른 후보를 계속 비교합니다.
방문 순서에 의존해서 규칙을 작성하지 마세요. 소스 순서는 `before`로 명시합니다.

## 바인딩 비교와 컬렉션 변환

`same_binding: {left: callee.object, right: "$array.id"}`는 이름 문자열이 아닌
Oxc의 심볼 ID를 비교합니다. 블록 스코프, 매개변수, 구조 분해, `var` 호이스팅을
분석하며, 이름을 해석할 수 없는 전역 식별자는 서로 같은 이름이어도 일치하지 않습니다.
속성 이름·별칭의 값·프로젝트 간 데이터 흐름을 같은 바인딩으로 취급하지 않습니다.

[함수형 변환 예제](../rules/examples/prefer-functional-transforms.yaml)는 다음을 YAML로 조합합니다.

1. 함수를 보고 단위로 선택하고 함수 본문에서 지역 빈 배열 선언을 캡처합니다.
2. 선언보다 뒤에 있는 반복문을 찾습니다.
3. 반복문 본문에서 캡처한 배열과 같은 바인딩의 `.push` 호출을 찾습니다.
4. 중첩 함수·클래스는 탐색 경계로 제외합니다.
5. 후보 함수 하나당 AI 요청 한 번으로 적합성을 판단합니다.

이는 순수성의 증명이 아닙니다. 배열 재대입·외부 전달·부수 효과·조기 종료는 AI가
문맥을 보고 판단합니다. 초기값이 있는 배열, 숫자·객체 누산, `forEach`, 계산된
`['push']` 접근 등을 검사하려면 해당 패턴을 YAML에 추가합니다.
전체 실패를 `filter`의 부분 성공으로 바꾸는 정책 변경은 별도의 요구가 있어야 합니다.

## AI와 실행 오류

`judge: {context: matched_node, criteria: ...}`를 추가하면 AST 후보만 모델에 보냅니다.
`matched_node`(기본), `enclosing_function`(자기 자신 포함, 없으면 매칭 노드),
`source_file`을 지원합니다. 응답이 `violation`이면 YAML 메시지로 보고하고,
`pass`이면 통과, `unknown`·모델 미설정·요청 실패이면 실행 오류입니다.
실제 소스가 설정한 모델 서버로 전송되며 기본 규칙은 모델을 사용하지 않습니다.

규칙 파일은 256 KiB, 쿼리 중첩은 32단계, 평가 경로의 캡처는 32개까지입니다.
파일당 규칙 하나에 평가·탐색 1,000,000단계 제한을 둡니다. 한도 초과나 AST JSON
인덱싱 깊이 제한 등 실행 실패는 종료 코드 2로 처리하며 정상 검사로 처리하지 않습니다.
지나치게 넓은 탐색은 종류·속성 조건과 `stop_at`으로 좁히세요.

## v1에서 전환

v1은 지원을 종료합니다. 전용 조건을 호환 구현으로 Rust에 남겨두지 않습니다.

| 이전 문법 | v2 표현 |
|---|---|
| `callee: foo` | `properties: {callee.type: Identifier, callee.name: foo}` |
| `callee: React.useEffect` | MemberExpression의 computed·object·property 속성 검사 |
| `where.contains` | `where.descendant.match` |
| `callback.index: 0` | `at.path: arguments.0` 및 함수 종류 검사 |
| `isStateSetter` | 구조 분해 선언 탐색 + useState 호출 속성 + same_binding 조합 |
| `hasLoopAccumulator` | 배열·반복문·push를 캡처·탐색·before·same_binding으로 조합 |

기본 규칙·예제·웹 데모는 모두 v2입니다. 기존 외부 YAML은 직접 전환해야 합니다.
Rust 호출부는 `RuleEngine::new(Vec<YamlRule>)`를 사용하며 `Rule` 트레이트를 구현하지 않습니다.

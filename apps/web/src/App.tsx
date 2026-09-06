import { useRef, useState } from 'react';

export type RuleInfo = { id: string; name: string; description: string; tag: string; yaml: string };
type Result = { exitCode: number; diagnostics: { ruleId: string; message: string; line: number; column: number }[]; output: string; syntaxError: boolean };

const examples = {
  effect: `import { useEffect, useState } from 'react';

export default function Counter() {
  const [count, setCount] = useState(0);

  useEffect(() => {
    setCount(1);
    console.log('Counter is ready');
  }, []);

  return (
    <button onClick={() => setCount(count + 1)}>
      Count: {count}
    </button>
  );
}
`,
  clean: `import { useState } from 'react';

export default function Counter() {
  const [count, setCount] = useState(0);

  return (
    <button onClick={() => setCount(count + 1)}>
      Count: {count}
    </button>
  );
}
`,
  alert: `export default function Welcome() {
  function greet() {
    alert('Hello, world!');
    console.log('Welcome button clicked');
  }

  return <button onClick={greet}>Say hello</button>;
}
`,
};

export default function App({ rules }: { rules: RuleInfo[] }) {
  const [source, setSource] = useState(examples.effect);
  const [selected, setSelected] = useState(rules.map(rule => rule.id));
  const [result, setResult] = useState<Result | null>(null);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [elapsed, setElapsed] = useState(0);
  const [stale, setStale] = useState(false);
  const editor = useRef<HTMLTextAreaElement>(null);
  const lineNumbers = useRef<HTMLDivElement>(null);

  function updateSource(value: string) { setSource(value); setStale(true); }
  function toggle(id: string) {
    setSelected(current => current.includes(id) ? current.filter(value => value !== id) : [...current, id]);
    setStale(true);
  }
  async function run() {
    if (busy || !source.trim() || !selected.length) return;
    setBusy(true); setError(''); setResult(null);
    const start = performance.now();
    try {
      const response = await fetch('/api/check', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ source, rules: selected }) });
      const data = await response.json();
      if (!response.ok) throw new Error(data.error || '검사에 실패했습니다.');
      setResult(data); setElapsed(Math.round(performance.now() - start)); setStale(false);
    } catch (reason) { setError(reason instanceof Error ? reason.message : '서버 연결을 확인하세요.'); }
    finally { setBusy(false); }
  }
  function jump(line: number) {
    const offset = source.split('\n').slice(0, line - 1).reduce((total, text) => total + text.length + 1, 0);
    editor.current?.focus(); editor.current?.setSelectionRange(offset, offset + (source.split('\n')[line - 1]?.length || 0));
    if (editor.current) editor.current.scrollTop = Math.max(0, (line - 4) * 25);
  }

  return <div className="shell">
    <header className="topbar"><a className="brand" href="/"><span className="brand-icon">a/</span> ai-lint <span className="brand-label">PLAYGROUND</span></a><span className="local-status"><i /> LOCAL WORKSPACE</span></header>
    <main>
      <section className="intro"><div><p className="eyebrow">YOUR CODE. YOUR RULES.</p><h1>코드를 넣고, 규칙을 확인하세요<span>.</span></h1><p className="subtitle">TypeScript 코드에 YAML 규칙을 적용하고, 놓치기 쉬운 패턴을 찾아보세요.</p></div><span className="version">v0.1 <span>EXPERIMENTAL</span></span></section>
      <div className="workspace">
        <section className="panel editor-panel" aria-label="코드 편집기">
          <div className="panel-heading"><div><span className="step">01</span><h2>코드 입력</h2></div><label className="sample-label">예제 <select disabled={busy} defaultValue="effect" onChange={event => updateSource(examples[event.target.value as keyof typeof examples])}><option value="effect">Effect와 상태 변경</option><option value="clean">위반 없는 코드</option><option value="alert">알림과 디버깅 로그</option></select></label></div>
          <div className="filebar"><span className="ts-badge">TS</span><span>playground.tsx</span><span className="file-note">편집 가능</span></div>
          <div className="editor-body"><div ref={lineNumbers} className="line-numbers" aria-hidden="true">{source.split('\n').map((_, index) => <div key={index}>{index + 1}</div>)}</div><textarea ref={editor} aria-label="TypeScript 코드" spellCheck={false} disabled={busy} value={source} onChange={event => updateSource(event.target.value)} onScroll={event => { if (lineNumbers.current) lineNumbers.current.scrollTop = event.currentTarget.scrollTop; }} onKeyDown={event => { if (event.key === 'Tab') { event.preventDefault(); const field = event.currentTarget; const position = field.selectionStart; updateSource(source.slice(0, position) + '  ' + source.slice(field.selectionEnd)); requestAnimationFrame(() => field.setSelectionRange(position + 2, position + 2)); } }} /></div>
          <div className="editor-footer"><span><i /> TypeScript + JSX</span><span>UTF-8 <span className="separator">/</span> {source.split('\n').length} lines</span></div>
        </section>
        <aside className="panel rules-panel"><div className="panel-heading"><div><span className="step">02</span><h2>검사 규칙</h2></div><span className="count">{selected.length} / {rules.length}</span></div><p className="rules-help">이 코드에 적용할 규칙을 선택하세요.</p><div className="rule-list">{rules.map(rule => <article className={`rule-card ${selected.includes(rule.id) ? 'selected' : ''}`} key={rule.id}><label><input type="checkbox" disabled={busy} checked={selected.includes(rule.id)} onChange={() => toggle(rule.id)} /><div><span className="rule-title">{rule.name}<span className="tag">{rule.tag}</span></span><code>{rule.id}</code><p>{rule.description}</p></div></label><details><summary>YAML 보기 <span>↗</span></summary><pre>{rule.yaml}</pre></details></article>)}</div><div className="run-area"><p>선택한 규칙으로 실제 Rust 엔진을 실행합니다.</p><button className="run-button" disabled={busy || !source.trim() || !selected.length} onClick={run}>{busy ? '검사 중…' : '코드 검사하기'}<span>{busy ? '◌' : '↗'}</span></button>{!selected.length && <small>규칙을 하나 이상 선택하세요.</small>}</div></aside>
      </div>
      <section className="panel results" aria-live="polite" aria-busy={busy}><div className="panel-heading"><div><span className="step">03</span><h2>검사 결과</h2>{result && <span className={`result-pill ${result.exitCode === 0 ? 'pass' : ''}`}>{result.exitCode === 0 ? '통과' : result.syntaxError ? '문법 오류' : `${result.diagnostics.length}개 발견`}</span>}</div>{result && <span className="timing">{stale ? '코드 또는 규칙이 변경되었습니다. 다시 검사하세요.' : `${elapsed} ms · ${result.diagnostics.length} findings`}</span>}</div>
        {busy ? <div className="empty"><span className="empty-icon">◌</span><p>코드를 분석하고 있습니다…</p></div> : error ? <div className="error-message" role="alert">{error}</div> : !result ? <div className="empty"><span className="empty-icon">⌁</span><div><strong>첫 검사를 실행해 보세요</strong><p>코드를 수정하고 규칙을 선택하면 이곳에 결과가 표시됩니다.</p></div></div> : result.exitCode === 0 ? <div className="empty success"><span className="empty-icon">✓</span><div><strong>선택한 규칙을 모두 통과했습니다</strong><p>입력한 코드에서 문법 오류와 규칙 위반을 찾지 못했습니다.</p></div></div> : result.syntaxError ? <pre className="syntax-output">{result.output}</pre> : <div className="diagnostics">{result.diagnostics.map((diagnostic, index) => <button disabled={stale} key={index} onClick={() => jump(diagnostic.line)}><span className="warning-dot">!</span><span><strong>{diagnostic.message}</strong><code>{diagnostic.ruleId}</code></span><span className="location">Ln {diagnostic.line}, Col {diagnostic.column} <span>↗</span></span></button>)}</div>}
      </section>
    </main><footer><span>ai-lint <span className="footer-muted">/ 코드의 문맥을 읽는 린터</span></span><span>Rust + Oxc <span className="separator">·</span> YAML rules</span></footer>
  </div>;
}

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import type { RuleInfo } from './App';
import './style.css';

const root = createRoot(document.getElementById('root')!);
root.render(<p className="loading">플레이그라운드를 준비하고 있습니다…</p>);
async function boot() {
  try {
    const response = await fetch('/api/rules');
    if (!response.ok) throw new Error('규칙 목록을 불러올 수 없습니다. 개발 서버를 확인하세요.');
    const data: { rules: RuleInfo[] } = await response.json();
    root.render(<StrictMode><App rules={data.rules} /></StrictMode>);
  } catch (error) {
    root.render(<div className="loading" role="alert"><p>{error instanceof Error ? error.message : '서버에 연결할 수 없습니다.'}</p><button onClick={() => void boot()}>다시 시도</button></div>);
  }
}
void boot();

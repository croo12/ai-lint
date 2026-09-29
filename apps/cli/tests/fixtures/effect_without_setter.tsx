import { useEffect, useState } from 'react';

export function Counter() {
  const [count, setCount] = useState(0);
  useEffect(() => {
    console.log(count);
  }, [count]);
  return <button onClick={() => setCount(count + 1)}>{count}</button>;
}

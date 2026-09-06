import { useEffect, useState } from 'react';

export function Counter() {
  const [count, setCount] = useState(0);
  useEffect(() => {
    setCount(1);
  }, []);
  return <div>{count}</div>;
}

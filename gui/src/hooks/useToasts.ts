import { useCallback, useRef, useState } from 'react';

export type ToastKind = 'success' | 'error' | 'info';
export interface Toast {
  id: number;
  kind: ToastKind;
  text: string;
}
export type PushToast = (kind: ToastKind, text: string) => void;

export function useToasts() {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(1);

  const dismiss = useCallback((id: number) => {
    setToasts((list) => list.filter((t) => t.id !== id));
  }, []);

  const push = useCallback<PushToast>(
    (kind, text) => {
      const id = nextId.current++;
      setToasts((list) => [...list.slice(-3), { id, kind, text }]);
      window.setTimeout(() => dismiss(id), kind === 'error' ? 9000 : 4500);
    },
    [dismiss],
  );

  return { toasts, push, dismiss };
}

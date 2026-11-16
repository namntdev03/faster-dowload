import { CircleCheck, Info, TriangleAlert, X } from 'lucide-react';
import type { Toast } from '../hooks/useToasts';

interface Props {
  toasts: Toast[];
  onDismiss: (id: number) => void;
}

const ICONS = { success: CircleCheck, error: TriangleAlert, info: Info };

export function Toasts({ toasts, onDismiss }: Props) {
  return (
    <div className="toasts" aria-live="polite">
      {toasts.map((t) => {
        const Icon = ICONS[t.kind];
        return (
          <div key={t.id} className={`toast ${t.kind}`} role={t.kind === 'error' ? 'alert' : 'status'}>
            <Icon size={17} aria-hidden="true" />
            <span>{t.text}</span>
            <button type="button" aria-label="Đóng thông báo" onClick={() => onDismiss(t.id)}>
              <X size={14} aria-hidden="true" />
            </button>
          </div>
        );
      })}
    </div>
  );
}

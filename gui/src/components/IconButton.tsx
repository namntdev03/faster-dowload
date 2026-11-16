import type { LucideIcon } from 'lucide-react';

interface Props {
  icon: LucideIcon;
  label: string;
  onClick: () => void;
  danger?: boolean;
}

/** Nút chỉ có icon, luôn có nhãn cho trình đọc màn hình và tooltip. */
export function IconButton({ icon: Icon, label, onClick, danger }: Props) {
  return (
    <button type="button" className={`icon-btn${danger ? ' danger' : ''}`} aria-label={label} title={label} onClick={onClick}>
      <Icon size={17} strokeWidth={1.9} aria-hidden="true" />
    </button>
  );
}

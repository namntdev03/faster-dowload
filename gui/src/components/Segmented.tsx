import type { ReactNode } from 'react';

export interface SegOption<T extends string> {
  value: T;
  label: string;
  icon?: ReactNode;
  hideLabelNarrow?: boolean;
}

interface Props<T extends string> {
  value: T;
  options: SegOption<T>[];
  onChange: (value: T) => void;
  label: string;
  stretch?: boolean;
  /** Chỉ hiện icon (nhãn vẫn có trong tooltip và cho trình đọc màn hình). */
  iconOnly?: boolean;
}

/** Nhóm nút chọn một (radiogroup) dùng cho chế độ, số kết nối, giao diện. */
export function Segmented<T extends string>({ value, options, onChange, label, stretch, iconOnly }: Props<T>) {
  return (
    <div className={`seg${stretch ? ' stretch' : ''}`} role="radiogroup" aria-label={label}>
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          role="radio"
          aria-checked={o.value === value}
          aria-label={iconOnly || o.hideLabelNarrow ? o.label : undefined}
          title={o.label}
          className="seg-btn"
          onClick={() => onChange(o.value)}
        >
          {o.icon}
          {!iconOnly && <span className={o.hideLabelNarrow ? 'hide-narrow' : undefined}>{o.label}</span>}
        </button>
      ))}
    </div>
  );
}

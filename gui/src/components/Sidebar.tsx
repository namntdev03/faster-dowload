import { ArrowDownToLine, CircleCheck, LayoutList, Monitor, Moon, Pause, Plus, Sun, TriangleAlert, Zap } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { fmtSpeed, MOD_KEY } from '../format';
import type { ThemeMode } from '../hooks/useSettings';
import { Segmented } from './Segmented';
import { SpeedGraph } from './SpeedGraph';

export type Filter = 'all' | 'running' | 'paused' | 'done' | 'error';

const ITEMS: { id: Filter; label: string; Icon: LucideIcon }[] = [
  { id: 'all', label: 'Tất cả', Icon: LayoutList },
  { id: 'running', label: 'Đang tải', Icon: ArrowDownToLine },
  { id: 'paused', label: 'Đã dừng', Icon: Pause },
  { id: 'done', label: 'Hoàn tất', Icon: CircleCheck },
  { id: 'error', label: 'Lỗi', Icon: TriangleAlert },
];

interface Props {
  filter: Filter;
  onFilter: (f: Filter) => void;
  counts: Record<Filter, number>;
  onAdd: () => void;
  theme: ThemeMode;
  onTheme: (t: ThemeMode) => void;
  speeds: number[];
  totalSpeed: number;
  maxActive: number;
  onMaxActive: (n: number) => void;
}

export function Sidebar({ filter, onFilter, counts, onAdd, theme, onTheme, speeds, totalSpeed, maxActive, onMaxActive }: Props) {
  return (
    <aside className="sidebar">
      <div className="brand">
        <span className="brand-mark" aria-hidden="true">
          <Zap size={17} strokeWidth={2.2} fill="currentColor" />
        </span>
        <span className="hide-narrow">FastDL</span>
      </div>

      <button type="button" className="btn btn-primary btn-block" onClick={onAdd} aria-label="Thêm URL" title="Thêm URL">
        <Plus size={16} aria-hidden="true" />
        <span className="hide-narrow">Thêm URL</span>
        <kbd className="hide-narrow">{MOD_KEY}N</kbd>
      </button>

      <nav aria-label="Bộ lọc lượt tải">
        <ul className="nav-list">
          {ITEMS.map(({ id, label, Icon }) => (
            <li key={id}>
              <button
                type="button"
                className={`nav-item${filter === id ? ' active' : ''}`}
                aria-current={filter === id ? 'page' : undefined}
                aria-label={`${label}, ${counts[id]} mục`}
                title={label}
                onClick={() => onFilter(id)}
              >
                <Icon size={16} aria-hidden="true" />
                <span className="nav-label hide-narrow">{label}</span>
                <span className="count">{counts[id]}</span>
              </button>
            </li>
          ))}
        </ul>
      </nav>

      <div className="sidebar-foot">
        <div className="speed-card hide-narrow">
          <div className="speed-label">Tốc độ tổng</div>
          <div className={`speed-value${totalSpeed > 0 ? '' : ' idle'}`}>
            {totalSpeed > 0 ? fmtSpeed(totalSpeed) : 'Đang rảnh'}
          </div>
          <SpeedGraph values={speeds} />
        </div>
        <div className="theme-row hide-narrow">
          <span className="speed-label" title="Số lượt tải chạy cùng lúc, các mục khác nằm chờ">
            Tải cùng lúc
          </span>
          <Segmented<string>
            label="Số lượt tải chạy cùng lúc"
            value={String(maxActive)}
            onChange={(v) => onMaxActive(Number(v))}
            options={['1', '2', '3', '5'].map((v) => ({ value: v, label: v }))}
          />
        </div>
        <div className="theme-row hide-narrow">
          <span className="speed-label">Giao diện</span>
          <Segmented<ThemeMode>
            label="Giao diện"
            iconOnly
            value={theme}
            onChange={onTheme}
            options={[
              { value: 'auto', label: 'Theo hệ thống', icon: <Monitor size={15} aria-hidden="true" /> },
              { value: 'dark', label: 'Tối', icon: <Moon size={15} aria-hidden="true" /> },
              { value: 'light', label: 'Sáng', icon: <Sun size={15} aria-hidden="true" /> },
            ]}
          />
        </div>
      </div>
    </aside>
  );
}

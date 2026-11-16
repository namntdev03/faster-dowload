import { useCallback, useEffect, useState } from 'react';

export type ThemeMode = 'auto' | 'dark' | 'light';
export type AddMode = 'auto' | 'file' | 'video';

export interface Settings {
  theme: ThemeMode;
  dir: string;
  connections: number;
  format: string;
  mode: AddMode;
}

const KEY = 'fastdl.settings.v1';
const DEFAULTS: Settings = { theme: 'auto', dir: '', connections: 16, format: 'bv*+ba/b', mode: 'auto' };

export const CONN_PRESETS = [4, 8, 16, 32];

/** Đưa số kết nối cũ (vd 24) về mức gần nhất mà giao diện có nút chọn. */
function snapConns(n: unknown): number {
  const v = typeof n === 'number' && n > 0 ? n : DEFAULTS.connections;
  return CONN_PRESETS.reduce((best, p) => (Math.abs(p - v) < Math.abs(best - v) ? p : best));
}

function load(): Settings {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return DEFAULTS;
    const s = { ...DEFAULTS, ...JSON.parse(raw) };
    return { ...s, connections: snapConns(s.connections) };
  } catch {
    return DEFAULTS;
  }
}

/** Cài đặt lưu trong localStorage và áp dụng giao diện sáng/tối. */
export function useSettings() {
  const [settings, setSettings] = useState<Settings>(load);

  const update = useCallback((patch: Partial<Settings>) => {
    setSettings((prev) => {
      const next = { ...prev, ...patch };
      try {
        localStorage.setItem(KEY, JSON.stringify(next));
      } catch {
        /* bỏ qua nếu không ghi được */
      }
      return next;
    });
  }, []);

  useEffect(() => {
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const apply = () => {
      const dark = settings.theme === 'auto' ? mq.matches : settings.theme === 'dark';
      document.documentElement.dataset.theme = dark ? 'dark' : 'light';
    };
    apply();
    mq.addEventListener('change', apply);
    return () => mq.removeEventListener('change', apply);
  }, [settings.theme]);

  return { settings, update };
}

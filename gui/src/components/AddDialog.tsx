import { useEffect, useMemo, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { readText } from '@tauri-apps/plugin-clipboard-manager';
import { Clipboard, FolderOpen } from 'lucide-react';
import { api, Kind } from '../api';
import { extractUrls, hostOf, looksLikeVideo, MOD_KEY } from '../format';
import type { AddMode, Settings } from '../hooks/useSettings';
import { Dialog } from './Dialog';
import { Segmented } from './Segmented';

const QUALITIES = [
  { label: 'Tốt nhất', value: 'bv*+ba/b' },
  { label: 'Tối đa 1080p', value: 'bv*[height<=1080]+ba/b' },
  { label: 'Tối đa 720p', value: 'bv*[height<=720]+ba/b' },
  { label: 'Tối đa 480p', value: 'bv*[height<=480]+ba/b' },
  { label: 'Chỉ âm thanh', value: 'ba/b' },
];

interface Props {
  initialText: string;
  settings: Settings;
  onSettings: (patch: Partial<Settings>) => void;
  onClose: () => void;
  onAdded: (count: number) => void;
}

export function AddDialog({ initialText, settings, onSettings, onClose, onAdded }: Props) {
  const [text, setText] = useState(initialText);
  const [mode, setMode] = useState<AddMode>(settings.mode);
  const [dir, setDir] = useState(settings.dir);
  const [format, setFormat] = useState(settings.format);
  const [conns, setConns] = useState(settings.connections);
  const [headers, setHeaders] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!dir) api.defaultDir().then(setDir).catch(() => undefined);
  }, []);

  const items = useMemo(
    () =>
      extractUrls(text).map((url) => ({
        url,
        kind: (mode === 'auto' ? (looksLikeVideo(url) ? 'video' : 'file') : mode) as Kind,
      })),
    [text, mode],
  );
  const hasVideo = items.some((i) => i.kind === 'video');

  const pickDir = async () => {
    try {
      const picked = await open({ directory: true, defaultPath: dir || undefined });
      if (typeof picked === 'string') setDir(picked);
    } catch (e) {
      setError(String(e));
    }
  };

  const pasteClipboard = async () => {
    try {
      const t = (await readText()) ?? '';
      if (extractUrls(t).length === 0) {
        setError('Clipboard chưa có link http(s).');
        return;
      }
      setText((cur) => (cur.trim() ? `${cur.trimEnd()}\n${t}` : t));
      setError('');
    } catch {
      setError(`Không đọc được clipboard. Hãy dán thủ công bằng ${MOD_KEY}V.`);
    }
  };

  const submit = async () => {
    if (busy) return;
    if (items.length === 0) {
      setError('Hãy dán ít nhất một link bắt đầu bằng http:// hoặc https://');
      return;
    }
    if (!dir.trim()) {
      setError('Chưa chọn thư mục lưu.');
      return;
    }
    setBusy(true);
    setError('');
    try {
      const hdrs = headers.split('\n').map((h) => h.trim()).filter(Boolean);
      for (const kind of ['file', 'video'] as Kind[]) {
        const urls = items.filter((i) => i.kind === kind).map((i) => i.url);
        if (urls.length > 0) await api.add({ urls, dir: dir.trim(), kind, format, connections: conns, headers: hdrs });
      }
      onSettings({ dir: dir.trim(), format, connections: conns, mode });
      onAdded(items.length);
      onClose();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  return (
    <Dialog title="Thêm lượt tải" onClose={onClose}>
      <div>
        <h2>Thêm lượt tải</h2>
        <p className="dialog-sub">Dán một hoặc nhiều link. App tự nhận diện file hay video.</p>
      </div>

      <div className="field">
        <div className="field-top">
          <label htmlFor="urls">Link (mỗi dòng một link)</label>
          <button type="button" className="btn btn-ghost" onClick={pasteClipboard}>
            <Clipboard size={14} aria-hidden="true" />
            Dán từ clipboard
          </button>
        </div>
        <textarea
          id="urls"
          rows={4}
          autoFocus
          value={text}
          placeholder="https://..."
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
              e.preventDefault();
              submit();
            }
          }}
        />
        {items.length > 0 && (
          <ul className="url-chips" aria-label="Link đã nhận diện">
            {items.slice(0, 6).map((i) => (
              <li key={i.url} className="url-chip">
                <b>{i.kind === 'video' ? 'Video' : 'File'}</b>
                <span>{hostOf(i.url)}</span>
              </li>
            ))}
            {items.length > 6 && (
              <li className="url-chip">
                <span>+{items.length - 6} link khác</span>
              </li>
            )}
          </ul>
        )}
      </div>

      <div className="field-row">
        <div className="field">
          <span className="field-label">Loại link</span>
          <Segmented<AddMode>
            label="Loại link"
            stretch
            value={mode}
            onChange={setMode}
            options={[
              { value: 'auto', label: 'Tự động' },
              { value: 'file', label: 'File' },
              { value: 'video', label: 'Video' },
            ]}
          />
        </div>
        {hasVideo && (
          <div className="field">
            <label htmlFor="quality">Chất lượng video</label>
            <select id="quality" value={format} onChange={(e) => setFormat(e.target.value)}>
              {QUALITIES.map((q) => (
                <option key={q.value} value={q.value}>
                  {q.label}
                </option>
              ))}
            </select>
          </div>
        )}
      </div>

      <div className="field">
        <span className="field-label">Số kết nối tối đa</span>
        <Segmented<string>
          label="Số kết nối tối đa"
          stretch
          value={String(conns)}
          onChange={(v) => setConns(Number(v))}
          options={[
            { value: '4', label: '4 · Nhẹ' },
            { value: '8', label: '8 · Vừa' },
            { value: '16', label: '16 · Nhanh' },
            { value: '32', label: '32 · Tối đa' },
          ]}
        />
        <p className="field-hint">Bắt đầu ít kết nối và tự tăng nếu tốc độ còn tăng. Một số server chặn nếu mở quá nhiều.</p>
      </div>

      <div className="field">
        <label htmlFor="dir">Thư mục lưu</label>
        <div className="dir-row">
          <input id="dir" type="text" value={dir} onChange={(e) => setDir(e.target.value)} />
          <button type="button" className="btn" onClick={pickDir}>
            <FolderOpen size={16} aria-hidden="true" />
            Chọn…
          </button>
        </div>
      </div>

      <details className="advanced">
        <summary>Nâng cao: header tùy chỉnh (Cookie, Referer…)</summary>
        <textarea
          aria-label="Header tùy chỉnh, mỗi dòng một header"
          rows={3}
          value={headers}
          placeholder={'Cookie: a=b\nReferer: https://...'}
          onChange={(e) => setHeaders(e.target.value)}
        />
      </details>

      {error && (
        <div className="field-error" role="alert">
          {error}
        </div>
      )}

      <div className="dialog-actions">
        <span className="hint">{MOD_KEY}Enter để bắt đầu</span>
        <button type="button" className="btn" onClick={onClose}>
          Hủy
        </button>
        <button type="button" className="btn btn-primary" disabled={busy || items.length === 0} onClick={submit}>
          {busy ? 'Đang thêm…' : `Bắt đầu tải${items.length > 1 ? ` (${items.length})` : ''}`}
        </button>
      </div>
    </Dialog>
  );
}

import { CircleCheck, Download, ExternalLink, Loader2, RefreshCw, TriangleAlert } from 'lucide-react';
import { useState } from 'react';
import { api } from '../api';
import { fmtBytes } from '../format';
import type { UpdateState } from '../hooks/useUpdate';
import { Dialog } from './Dialog';

interface Props {
  current: string;
  state: UpdateState;
  dir: string;
  onCheck: () => void;
  onClose: () => void;
  onNotify: (kind: 'success' | 'error', text: string) => void;
}

export function UpdateDialog({ current, state, dir, onCheck, onClose, onNotify }: Props) {
  const [starting, setStarting] = useState(false);

  async function download() {
    if (state.phase !== 'available' || !state.release.asset) return;
    setStarting(true);
    try {
      const target = dir.trim() || (await api.defaultDir());
      await api.add({ urls: [state.release.asset.url], dir: target, kind: 'file', format: '', connections: 8, headers: [] });
      onNotify('success', `Đang tải ${state.release.asset.name}, mở file sau khi xong để cài đặt`);
      onClose();
    } catch (e) {
      onNotify('error', String(e));
    } finally {
      setStarting(false);
    }
  }

  const openPage = (url: string) => api.openReleasePage(url).catch((e) => onNotify('error', String(e)));

  return (
    <Dialog title="Cập nhật FastDL" onClose={onClose} width={480}>
      <div>
        <h2>Cập nhật FastDL</h2>
        <p className="dialog-sub">Phiên bản đang dùng: {current ? `v${current}` : '—'}</p>
      </div>

      {state.phase === 'checking' || state.phase === 'idle' ? (
        <div className="update-status" role="status">
          <Loader2 size={20} className="spin" aria-hidden="true" />
          <span>Đang kiểm tra bản mới trên GitHub…</span>
        </div>
      ) : null}

      {state.phase === 'latest' && (
        <div className="update-status ok" role="status">
          <CircleCheck size={20} aria-hidden="true" />
          <span>Bạn đang dùng bản mới nhất.</span>
        </div>
      )}

      {state.phase === 'error' && (
        <div className="update-status bad" role="alert">
          <TriangleAlert size={20} aria-hidden="true" />
          <span>{state.message}</span>
        </div>
      )}

      {state.phase === 'available' && (
        <>
          <div className="update-status ok" role="status">
            <Download size={20} aria-hidden="true" />
            <span>
              Có bản mới <b>v{state.release.version}</b>
              {state.release.publishedAt && <> · {new Date(state.release.publishedAt).toLocaleDateString('vi-VN')}</>}
            </span>
          </div>
          {state.release.notes.trim() && (
            <div className="field">
              <span className="field-label">Có gì mới</span>
              <pre className="release-notes">{state.release.notes.trim()}</pre>
            </div>
          )}
          {state.release.asset ? (
            <p className="field-hint">
              File cài đặt cho máy này: {state.release.asset.name}
              {state.release.asset.size > 0 && ` (${fmtBytes(state.release.asset.size)})`}
            </p>
          ) : (
            <p className="field-hint">Chưa có file cài đặt hợp với máy này, hãy mở trang phát hành để chọn.</p>
          )}
        </>
      )}

      <div className="dialog-actions">
        {state.phase === 'available' ? (
          <>
            <button type="button" className="btn" onClick={() => openPage(state.release.pageUrl)}>
              <ExternalLink size={15} aria-hidden="true" />
              Trang GitHub
            </button>
            {state.release.asset && (
              <button type="button" className="btn btn-primary" onClick={download} disabled={starting} autoFocus>
                <Download size={15} aria-hidden="true" />
                {starting ? 'Đang thêm…' : 'Tải bản mới'}
              </button>
            )}
          </>
        ) : (
          <>
            <button type="button" className="btn" onClick={onCheck} disabled={state.phase === 'checking'}>
              <RefreshCw size={15} aria-hidden="true" />
              Kiểm tra lại
            </button>
            <button type="button" className="btn btn-primary" onClick={onClose} autoFocus>
              Đóng
            </button>
          </>
        )}
      </div>
    </Dialog>
  );
}

import { useState } from 'react';
import { ArrowDownToLine, CircleCheck, FolderOpen, Pause, Play, Trash2, TriangleAlert } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { Job, Status } from '../api';
import { fmtBytes, fmtEta, fmtSpeed, hostOf } from '../format';
import { FileIcon } from './FileIcon';
import { IconButton } from './IconButton';

const STATUS: Record<Status, { text: string; Icon: LucideIcon; cls: string }> = {
  running: { text: 'Đang tải', Icon: ArrowDownToLine, cls: 'is-running' },
  pausing: { text: 'Đang dừng', Icon: Pause, cls: 'is-paused' },
  paused: { text: 'Đã dừng', Icon: Pause, cls: 'is-paused' },
  done: { text: 'Hoàn tất', Icon: CircleCheck, cls: 'is-done' },
  error: { text: 'Lỗi', Icon: TriangleAlert, cls: 'is-error' },
};

interface Props {
  job: Job;
  onPause: () => void;
  onResume: () => void;
  onReveal: () => void;
  onRemove: () => void;
}

export function JobCard({ job, onPause, onResume, onReveal, onRemove }: Props) {
  const [expanded, setExpanded] = useState(false);
  const st = STATUS[job.status];
  const active = job.status === 'running';
  const indeterminate = active && job.total === 0;
  const pct = job.status === 'done' ? 100 : job.total > 0 ? Math.min(100, (job.downloaded / job.total) * 100) : 0;
  const title = job.name || job.url;
  const canResume = job.status === 'paused' || job.status === 'error';
  const isError = job.status === 'error';
  const detail = isError ? job.error || 'Lỗi không rõ nguyên nhân.' : active ? job.note : '';

  return (
    <article className={`card ${st.cls}`} aria-label={title}>
      <FileIcon name={job.name} kind={job.kind} />

      <div className="card-main">
        <div className="card-head">
          <h3 className="card-title" title={job.url}>
            {title}
          </h3>
          <span className={`chip ${st.cls}`}>
            <st.Icon size={13} aria-hidden="true" />
            {st.text}
          </span>
        </div>

        <div className="card-meta">
          {hostOf(job.url)} · {job.kind === 'video' ? 'Video' : 'File'}
          {job.total > 0 ? ` · ${fmtBytes(job.total)}` : ''}
        </div>

        <div
          className={`bar${indeterminate ? ' indeterminate' : ''}`}
          role="progressbar"
          aria-label={`Tiến độ ${title}`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={indeterminate ? undefined : Math.round(pct)}
        >
          <div className="bar-fill" style={indeterminate ? undefined : { width: `${pct}%` }} />
        </div>

        <div className="card-stats">
          <span className="num">{pct.toFixed(1)}%</span>
          {job.total > 0 && (
            <span>
              {fmtBytes(job.downloaded)} / {fmtBytes(job.total)}
            </span>
          )}
          {active && <span>{fmtSpeed(job.speed)}</span>}
          {active && job.total > 0 && job.speed > 0 && <span>Còn {fmtEta(job.total - job.downloaded, job.speed)}</span>}
          {active && job.conns > 0 && <span>{job.conns} kết nối</span>}
        </div>

        {detail && (
          <div className={`card-detail${isError ? ' is-error' : ''}`} role={isError ? 'alert' : undefined}>
            <p className={expanded ? undefined : 'clamp'}>{detail}</p>
            {detail.length > 110 && (
              <button type="button" className="link-btn" onClick={() => setExpanded(!expanded)}>
                {expanded ? 'Thu gọn' : 'Xem đầy đủ'}
              </button>
            )}
          </div>
        )}
      </div>

      <div className="card-actions">
        {active && <IconButton icon={Pause} label="Dừng" onClick={onPause} />}
        {canResume && <IconButton icon={Play} label="Tiếp tục" onClick={onResume} />}
        <IconButton icon={FolderOpen} label="Mở thư mục" onClick={onReveal} />
        <IconButton icon={Trash2} label="Xóa" onClick={onRemove} danger />
      </div>
    </article>
  );
}

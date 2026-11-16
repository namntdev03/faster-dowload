import { useEffect, useMemo, useRef, useState } from 'react';
import { Check, Pause, Play, Search } from 'lucide-react';
import type { Job } from './api';
import { AddDialog } from './components/AddDialog';
import { EmptyState } from './components/EmptyState';
import { JobCard } from './components/JobCard';
import { RemoveDialog } from './components/RemoveDialog';
import { Filter, Sidebar } from './components/Sidebar';
import { Toasts } from './components/Toasts';
import { extractUrls } from './format';
import { useJobs } from './hooks/useJobs';
import { useSettings } from './hooks/useSettings';
import { useToasts } from './hooks/useToasts';

const TITLES: Record<Filter, string> = {
  all: 'Tất cả lượt tải',
  running: 'Đang tải',
  paused: 'Đã dừng',
  done: 'Hoàn tất',
  error: 'Lỗi',
};

function matches(job: Job, f: Filter): boolean {
  switch (f) {
    case 'all':
      return true;
    case 'running':
      return job.status === 'running' || job.status === 'pausing';
    case 'paused':
      return job.status === 'paused';
    case 'done':
      return job.status === 'done';
    case 'error':
      return job.status === 'error';
  }
}

export default function App() {
  const { settings, update } = useSettings();
  const { toasts, push, dismiss } = useToasts();
  const { jobs, speeds, actions } = useJobs(push);
  const [filter, setFilter] = useState<Filter>('all');
  const [query, setQuery] = useState('');
  const [addText, setAddText] = useState<string | null>(null); // null = hộp thoại đóng
  const [removing, setRemoving] = useState<Job | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  // Phím tắt: Cmd/Ctrl+N thêm link, Cmd/Ctrl+F tìm kiếm; dán link ở bất kỳ đâu sẽ mở hộp thêm link.
  useEffect(() => {
    // Không mở chồng hộp thoại khi đang có một hộp thoại khác.
    const dialogOpen = () => document.querySelector('.overlay') !== null;
    const onKey = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || dialogOpen()) return;
      const k = e.key.toLowerCase();
      if (k === 'n') {
        e.preventDefault();
        setAddText('');
      } else if (k === 'f') {
        e.preventDefault();
        searchRef.current?.focus();
      }
    };
    const onPaste = (e: ClipboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (dialogOpen() || (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA'))) return;
      const text = e.clipboardData?.getData('text') ?? '';
      if (extractUrls(text).length > 0) {
        e.preventDefault();
        setAddText(text);
      }
    };
    window.addEventListener('keydown', onKey);
    window.addEventListener('paste', onPaste);
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('paste', onPaste);
    };
  }, []);

  const counts = useMemo(() => {
    const c: Record<Filter, number> = { all: jobs.length, running: 0, paused: 0, done: 0, error: 0 };
    for (const j of jobs) {
      for (const f of ['running', 'paused', 'done', 'error'] as Filter[]) if (matches(j, f)) c[f]++;
    }
    return c;
  }, [jobs]);

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return jobs.filter((j) => matches(j, filter) && (!q || `${j.name} ${j.url}`.toLowerCase().includes(q))).reverse();
  }, [jobs, filter, query]);

  const totalSpeed = speeds.length > 0 ? speeds[speeds.length - 1] : 0;

  return (
    <div className="shell">
      <Sidebar
        filter={filter}
        onFilter={setFilter}
        counts={counts}
        onAdd={() => setAddText('')}
        theme={settings.theme}
        onTheme={(theme) => update({ theme })}
        speeds={speeds}
        totalSpeed={totalSpeed}
      />

      <div className="main">
        <header className="main-head">
          <h1>{TITLES[filter]}</h1>
          <div className="search">
            <Search size={15} aria-hidden="true" />
            <input
              ref={searchRef}
              type="search"
              aria-label="Tìm theo tên hoặc link"
              placeholder="Tìm theo tên hoặc link…"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
          <div className="head-actions">
            <button type="button" className="btn" onClick={actions.resumeAll} disabled={counts.paused + counts.error === 0}>
              <Play size={15} aria-hidden="true" />
              Tiếp tục tất cả
            </button>
            <button type="button" className="btn" onClick={actions.pauseAll} disabled={counts.running === 0}>
              <Pause size={15} aria-hidden="true" />
              Dừng tất cả
            </button>
            <button type="button" className="btn btn-ghost" onClick={actions.clearDone} disabled={counts.done === 0}>
              <Check size={15} aria-hidden="true" />
              Dọn mục đã xong
            </button>
          </div>
        </header>

        <main className="list">
          {visible.length === 0 ? (
            <EmptyState filtered={jobs.length > 0} onAdd={() => setAddText('')} />
          ) : (
            visible.map((job) => (
              <JobCard
                key={job.id}
                job={job}
                onPause={() => actions.pause(job.id)}
                onResume={() => actions.resume(job.id)}
                onReveal={() => actions.reveal(job.id)}
                onRemove={() => setRemoving(job)}
              />
            ))
          )}
        </main>
      </div>

      {addText !== null && (
        <AddDialog
          initialText={addText}
          settings={settings}
          onSettings={update}
          onClose={() => setAddText(null)}
          onAdded={(n) => {
            setFilter('all');
            push('success', n === 1 ? 'Đã thêm 1 lượt tải' : `Đã thêm ${n} lượt tải`);
          }}
        />
      )}

      {removing && (
        <RemoveDialog
          job={removing}
          onCancel={() => setRemoving(null)}
          onConfirm={(deleteFile) => {
            actions.remove(removing.id, deleteFile);
            setRemoving(null);
          }}
        />
      )}

      <Toasts toasts={toasts} onDismiss={dismiss} />
    </div>
  );
}

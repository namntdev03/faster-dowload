import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { api, Job, Status } from '../api';
import type { PushToast } from './useToasts';

const HISTORY = 60; // 60 mẫu x 0.5s = 30 giây

/** Hỏi backend mỗi 0.5s, báo toast khi xong/lỗi, giữ lịch sử tốc độ cho biểu đồ. */
export function useJobs(push: PushToast) {
  const [jobs, setJobs] = useState<Job[]>([]);
  const [speeds, setSpeeds] = useState<number[]>([]);
  const prev = useRef(new Map<number, Status>());
  const jobsRef = useRef<Job[]>([]);
  const failing = useRef(false);
  const firstLoad = useRef(true); // lần đầu (hoặc sau HMR) chỉ ghi nhận trạng thái, không bắn toast
  const pushRef = useRef(push);
  pushRef.current = push;

  const refresh = useCallback(async () => {
    try {
      const list = await api.list();
      failing.current = false;
      const seen = new Set<number>();
      for (const j of list) {
        seen.add(j.id);
        if (prev.current.get(j.id) === j.status) continue;
        prev.current.set(j.id, j.status);
        if (firstLoad.current) continue;
        const label = j.name || j.url;
        if (j.status === 'done') pushRef.current('success', `Đã tải xong: ${label}`);
        if (j.status === 'error') pushRef.current('error', `Lỗi khi tải ${label}: ${j.error}`);
      }
      firstLoad.current = false;
      for (const id of Array.from(prev.current.keys())) if (!seen.has(id)) prev.current.delete(id);
      jobsRef.current = list;
      setJobs(list);
      const total = list.filter((j) => j.status === 'running').reduce((s, j) => s + j.speed, 0);
      setSpeeds((s) => [...s.slice(-(HISTORY - 1)), total]);
    } catch (e) {
      if (!failing.current) pushRef.current('error', `Không đọc được danh sách tải: ${String(e)}`);
      failing.current = true;
    }
  }, []);

  useEffect(() => {
    refresh();
    const t = setInterval(refresh, 500);
    return () => clearInterval(t);
  }, [refresh]);

  const run = useCallback(
    async (fn: () => Promise<unknown>) => {
      try {
        await fn();
      } catch (e) {
        pushRef.current('error', String(e));
      }
      await refresh();
    },
    [refresh],
  );

  const actions = useMemo(
    () => ({
      pause: (id: number) => run(() => api.pause(id)),
      resume: (id: number) => run(() => api.resume(id)),
      reveal: (id: number) => run(() => api.reveal(id)),
      remove: (id: number, deleteFile: boolean) => run(() => api.remove(id, deleteFile)),
      pauseAll: () =>
        run(() => Promise.all(jobsRef.current.filter((j) => j.status === 'running').map((j) => api.pause(j.id)))),
      resumeAll: () =>
        run(() =>
          Promise.all(
            jobsRef.current.filter((j) => j.status === 'paused' || j.status === 'error').map((j) => api.resume(j.id)),
          ),
        ),
      clearDone: async () => {
        const done = jobsRef.current.filter((j) => j.status === 'done');
        await run(() => Promise.all(done.map((j) => api.remove(j.id, false))));
        if (done.length > 0) pushRef.current('info', `Đã dọn ${done.length} mục đã xong (file vẫn còn trên ổ đĩa)`);
      },
    }),
    [run],
  );

  return { jobs, speeds, actions };
}

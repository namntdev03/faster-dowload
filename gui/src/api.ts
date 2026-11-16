import { invoke } from '@tauri-apps/api/core';

export type Kind = 'file' | 'video';
export type Status = 'queued' | 'running' | 'pausing' | 'paused' | 'done' | 'error';

export interface Job {
  id: number;
  url: string;
  name: string;
  kind: Kind;
  status: Status;
  downloaded: number;
  total: number;
  speed: number;
  conns: number;
  error: string;
  note: string;
  dir: string;
}

export interface AddReq {
  urls: string[];
  dir: string;
  kind: Kind;
  format: string;
  connections: number;
  headers: string[];
}

export const api = {
  list: () => invoke<Job[]>('list_jobs'),
  defaultDir: () => invoke<string>('default_dir'),
  add: (req: AddReq) => invoke<number[]>('add_download', { req }),
  pause: (id: number) => invoke<void>('pause_job', { id }),
  resume: (id: number) => invoke<void>('resume_job', { id }),
  remove: (id: number, deleteFile: boolean) => invoke<void>('remove_job', { id, deleteFile }),
  reveal: (id: number) => invoke<void>('reveal_job', { id }),
  setMaxActive: (n: number) => invoke<void>('set_max_active', { n }),
};

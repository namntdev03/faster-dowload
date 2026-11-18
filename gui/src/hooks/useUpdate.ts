import { getVersion } from '@tauri-apps/api/app';
import { useCallback, useEffect, useRef, useState } from 'react';
import { api } from '../api';

const REPO = 'namntdev03/faster-dowload';
const API_URL = `https://api.github.com/repos/${REPO}/releases/latest`;

export interface ReleaseAsset {
  name: string;
  url: string;
  size: number;
}

export interface ReleaseInfo {
  version: string;
  name: string;
  notes: string;
  pageUrl: string;
  publishedAt: string;
  /** File cài đặt hợp với máy này (nếu tìm được). */
  asset: ReleaseAsset | null;
}

export type UpdateState =
  | { phase: 'idle' }
  | { phase: 'checking' }
  | { phase: 'latest'; checkedAt: number }
  | { phase: 'available'; release: ReleaseInfo }
  | { phase: 'error'; message: string };

/** So sánh "0.2.10" với "0.2.9": >0 nếu a mới hơn b. */
export function compareVersions(a: string, b: string): number {
  const pa = a.replace(/^v/i, '').split('.').map((x) => parseInt(x, 10) || 0);
  const pb = b.replace(/^v/i, '').split('.').map((x) => parseInt(x, 10) || 0);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (d !== 0) return d;
  }
  return 0;
}

/** Chọn file cài đặt hợp hệ điều hành và kiến trúc CPU. */
function pickAsset(assets: ReleaseAsset[], os: string, arch: string): ReleaseAsset | null {
  const has = (a: ReleaseAsset, ...keys: string[]) => keys.some((k) => a.name.toLowerCase().includes(k));
  const ends = (a: ReleaseAsset, ...exts: string[]) => exts.some((e) => a.name.toLowerCase().endsWith(e));
  const arm = arch === 'aarch64' || arch === 'arm64';
  if (os === 'macos') {
    const dmgs = assets.filter((a) => ends(a, '.dmg'));
    return dmgs.find((a) => (arm ? has(a, 'aarch64', 'arm64') : has(a, 'x64', 'x86_64'))) ?? dmgs[0] ?? null;
  }
  if (os === 'windows') {
    return assets.find((a) => ends(a, '.exe') && !has(a, 'cli')) ?? assets.find((a) => ends(a, '.msi')) ?? null;
  }
  return assets.find((a) => ends(a, '.appimage')) ?? assets.find((a) => ends(a, '.deb')) ?? null;
}

/** Kiểm tra bản phát hành mới nhất trên GitHub Releases và so với phiên bản đang chạy. */
export function useUpdate() {
  const [current, setCurrent] = useState('');
  const [state, setState] = useState<UpdateState>({ phase: 'idle' });
  const busy = useRef(false);

  useEffect(() => {
    getVersion().then(setCurrent).catch(() => setCurrent(''));
  }, []);

  const check = useCallback(async () => {
    if (busy.current) return;
    busy.current = true;
    setState({ phase: 'checking' });
    try {
      const [version, [os, arch]] = await Promise.all([getVersion(), api.platformInfo()]);
      const res = await fetch(API_URL, { headers: { Accept: 'application/vnd.github+json' } });
      if (res.status === 404) {
        setState({ phase: 'latest', checkedAt: Date.now() }); // chưa có bản phát hành nào
        return;
      }
      if (!res.ok) throw new Error(res.status === 403 ? 'GitHub tạm giới hạn truy cập, thử lại sau ít phút' : `GitHub trả về lỗi ${res.status}`);
      const r = await res.json();
      const assets: ReleaseAsset[] = (r.assets ?? []).map((a: { name: string; browser_download_url: string; size: number }) => ({
        name: a.name,
        url: a.browser_download_url,
        size: a.size,
      }));
      const latest = String(r.tag_name ?? '');
      if (compareVersions(latest, version) > 0) {
        setState({
          phase: 'available',
          release: {
            version: latest.replace(/^v/i, ''),
            name: r.name || latest,
            notes: r.body ?? '',
            pageUrl: r.html_url,
            publishedAt: r.published_at ?? '',
            asset: pickAsset(assets, os, arch),
          },
        });
      } else {
        setState({ phase: 'latest', checkedAt: Date.now() });
      }
    } catch (e) {
      setState({ phase: 'error', message: e instanceof Error && e.message ? e.message : 'Không kết nối được GitHub' });
    } finally {
      busy.current = false;
    }
  }, []);

  // Tự kiểm tra một lần khi mở app (im lặng, chỉ hiện chấm báo nếu có bản mới).
  useEffect(() => {
    check();
  }, [check]);

  return { current, state, check };
}

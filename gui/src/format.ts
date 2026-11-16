const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];

export function fmtBytes(n: number): string {
  if (!n || n < 0) return '—';
  let v = n;
  let i = 0;
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(i === 0 ? 0 : 1)} ${UNITS[i]}`;
}

export function fmtSpeed(n: number): string {
  return n > 0 ? `${fmtBytes(n)}/s` : '—';
}

export function fmtEta(remaining: number, speed: number): string {
  if (speed <= 0 || remaining <= 0) return '—';
  const s = Math.round(remaining / speed);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  const pad = (x: number) => String(x).padStart(2, '0');
  return h > 0 ? `${h}:${pad(m)}:${pad(sec)}` : `${pad(m)}:${pad(sec)}`;
}

const VIDEO_HOSTS = [
  'youtube.com', 'youtu.be', 'vimeo.com', 'facebook.com', 'fb.watch', 'tiktok.com',
  'twitter.com', 'x.com', 'instagram.com', 'dailymotion.com', 'bilibili.com', 'twitch.tv',
];

/** Đoán link là trang video (cần yt-dlp) hay file trực tiếp. */
export function looksLikeVideo(url: string): boolean {
  try {
    const host = new URL(url.trim()).hostname.toLowerCase();
    return VIDEO_HOSTS.some((h) => host === h || host.endsWith('.' + h));
  } catch {
    return false;
  }
}

export function hostOf(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, '');
  } catch {
    return url;
  }
}

export function extOf(name: string): string {
  const i = name.lastIndexOf('.');
  return i > 0 ? name.slice(i + 1).toLowerCase() : '';
}

/** Lấy mọi link http(s) trong đoạn văn bản, bỏ trùng. */
export function extractUrls(text: string): string[] {
  const found = text.match(/https?:\/\/[^\s"'<>]+/gi) ?? [];
  return Array.from(new Set(found));
}

export const IS_MAC = /Mac/i.test(navigator.userAgent);
export const MOD_KEY = IS_MAC ? '⌘' : 'Ctrl+';

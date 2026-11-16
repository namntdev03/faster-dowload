import { isPermissionGranted, requestPermission, sendNotification } from '@tauri-apps/plugin-notification';

let allowed: boolean | null = null;

/** Thông báo của hệ điều hành, chỉ khi cửa sổ app không được focus (đang focus thì toast là đủ). */
export async function notify(title: string, body: string) {
  if (document.hasFocus()) return;
  try {
    if (allowed === null) {
      allowed = (await isPermissionGranted()) || (await requestPermission()) === 'granted';
    }
    if (allowed) sendNotification({ title, body: body.length > 180 ? `${body.slice(0, 177)}…` : body });
  } catch {
    allowed = false; // không có quyền/không hỗ trợ: im lặng, toast vẫn hiện
  }
}

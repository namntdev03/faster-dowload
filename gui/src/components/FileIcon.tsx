import { Archive, File as FileGeneric, FileText, Film, Image as ImageIcon, Music, Package } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { Kind } from '../api';
import { extOf } from '../format';

const MAP: Record<string, LucideIcon> = {};
const add = (icon: LucideIcon, exts: string) => exts.split(' ').forEach((e) => (MAP[e] = icon));
add(Film, 'mp4 mkv webm mov avi flv m4v ts');
add(Music, 'mp3 m4a flac wav ogg opus aac');
add(Archive, 'zip rar 7z gz tgz bz2 xz tar');
add(Package, 'pkg dmg iso exe msi deb rpm apk appimage');
add(FileText, 'pdf doc docx xls xlsx ppt pptx txt epub');
add(ImageIcon, 'png jpg jpeg gif webp svg heic');

/** Icon theo đuôi file; chưa biết tên thì dựa vào loại (video/file). */
export function FileIcon({ name, kind }: { name: string; kind: Kind }) {
  const Icon = MAP[extOf(name)] ?? (kind === 'video' ? Film : FileGeneric);
  return (
    <span className="file-icon" aria-hidden="true">
      <Icon size={20} strokeWidth={1.75} />
    </span>
  );
}

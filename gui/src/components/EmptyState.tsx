import { ArrowDownToLine, Inbox, Plus } from 'lucide-react';
import { MOD_KEY } from '../format';

interface Props {
  /** true khi danh sách trống chỉ vì bộ lọc hoặc từ khóa tìm kiếm. */
  filtered: boolean;
  onAdd: () => void;
}

export function EmptyState({ filtered, onAdd }: Props) {
  if (filtered) {
    return (
      <div className="empty">
        <span className="empty-icon">
          <Inbox size={28} aria-hidden="true" />
        </span>
        <h2>Không có mục nào ở đây</h2>
        <p>Thử đổi bộ lọc bên trái hoặc xóa từ khóa tìm kiếm.</p>
      </div>
    );
  }
  return (
    <div className="empty">
      <span className="empty-icon">
        <ArrowDownToLine size={28} aria-hidden="true" />
      </span>
      <h2>Chưa có lượt tải nào</h2>
      <p>
        Thêm một link để bắt đầu. Mẹo: copy link rồi nhấn <kbd>{MOD_KEY}V</kbd> ngay trong cửa sổ này, hộp thêm link sẽ tự mở.
      </p>
      <button type="button" className="btn btn-primary" onClick={onAdd}>
        <Plus size={16} aria-hidden="true" />
        Thêm URL
      </button>
    </div>
  );
}

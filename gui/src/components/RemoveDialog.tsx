import { useState } from 'react';
import type { Job } from '../api';
import { Dialog } from './Dialog';

interface Props {
  job: Job;
  onCancel: () => void;
  onConfirm: (deleteFile: boolean) => void;
}

export function RemoveDialog({ job, onCancel, onConfirm }: Props) {
  const [alsoFile, setAlsoFile] = useState(false);
  const done = job.status === 'done';

  return (
    <Dialog title="Xóa lượt tải" onClose={onCancel} width={440}>
      <div>
        <h2>Xóa lượt tải này?</h2>
        <p className="dialog-sub">{job.name || job.url}</p>
      </div>

      {done ? (
        <label className="check">
          <input type="checkbox" checked={alsoFile} onChange={(e) => setAlsoFile(e.target.checked)} />
          <span>
            Đồng thời xóa file đã tải khỏi ổ đĩa
            <br />
            <span className="field-hint">Không thể hoàn tác.</span>
          </span>
        </label>
      ) : (
        <p className="field-hint">Phần đã tải dở sẽ bị xóa. Bạn có thể thêm lại link để tải từ đầu.</p>
      )}

      <div className="dialog-actions">
        <button type="button" className="btn" autoFocus onClick={onCancel}>
          Hủy
        </button>
        <button type="button" className="btn btn-danger" onClick={() => onConfirm(done && alsoFile)}>
          Xóa
        </button>
      </div>
    </Dialog>
  );
}

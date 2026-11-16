/** Biểu đồ đường nhỏ cho tốc độ 30 giây gần nhất (chỉ trang trí, số liệu có ở chữ bên cạnh). */
export function SpeedGraph({ values }: { values: number[] }) {
  const W = 100;
  const H = 30;
  if (!values.some((v) => v > 0)) {
    // Chưa có tốc độ: chỉ vẽ đường nền mờ, không vẽ vạch đậm dễ nhầm là lỗi.
    return (
      <svg className="graph" viewBox="0 0 100 30" preserveAspectRatio="none" aria-hidden="true">
        <line x1="0" y1="29" x2="100" y2="29" stroke="currentColor" strokeWidth="1" strokeDasharray="3 3" opacity="0.3" vectorEffect="non-scaling-stroke" />
      </svg>
    );
  }
  const max = Math.max(...values, 1);
  const step = values.length > 1 ? W / (values.length - 1) : W;
  const pts = values.map((v, i) => `${(i * step).toFixed(1)},${(H - 2 - (v / max) * (H - 4)).toFixed(1)}`);
  const line = pts.join(' ');
  const area = values.length > 1 ? `0,${H} ${line} ${W},${H}` : '';
  return (
    <svg className="graph" viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" aria-hidden="true">
      {area && <polygon points={area} fill="currentColor" opacity="0.14" />}
      {values.length > 1 && (
        <polyline points={line} fill="none" stroke="currentColor" strokeWidth="1.6" vectorEffect="non-scaling-stroke" />
      )}
    </svg>
  );
}

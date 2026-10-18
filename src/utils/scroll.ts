/// Alt + 滚轮：把纵向滚轮转换为横向滚动。
/// 仅当容器确实存在横向溢出时生效，避免影响正常纵向滚动。
export function altWheelToHorizontal(event: WheelEvent) {
  if (!event.altKey) return;
  const el = event.currentTarget as HTMLElement | null;
  if (!el || el.scrollWidth <= el.clientWidth + 1) return;
  if (Math.abs(event.deltaX) > Math.abs(event.deltaY)) return;
  const delta = event.deltaMode === 1 ? event.deltaY * 16 : event.deltaY;
  if (!delta) return;
  event.preventDefault();
  el.scrollLeft += delta;
}

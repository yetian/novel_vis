/// 将 Tauri 命令返回的错误对象 `{ code, message }` 或任意异常转换为可读文本。
/// 避免在界面上出现 `[object Object]`。
export interface AppErrorLike {
  code?: string;
  message?: string;
}

export function errText(e: unknown): string {
  if (e == null) return "未知错误";
  if (typeof e === "string") return e;
  if (typeof e === "object") {
    const obj = e as AppErrorLike & { toString?: () => string };
    if (typeof obj.message === "string" && obj.message.trim()) {
      return obj.code ? `${obj.message}（${obj.code}）` : obj.message;
    }
    if (obj.toString && obj.toString !== Object.prototype.toString) return obj.toString();
  }
  return String(e);
}

export function errCode(e: unknown): string | null {
  if (e && typeof e === "object" && "code" in e) {
    const code = (e as AppErrorLike).code;
    return typeof code === "string" ? code : null;
  }
  return null;
}

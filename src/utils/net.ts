// 网关监听地址 → 浏览器可访问地址。
// `0.0.0.0` / `::` 代表“本机所有网卡”，浏览器无法访问（会报 ERR_ADDRESS_INVALID），
// 这里统一转成 `127.0.0.1`；其它局域网 IP（外部访问场景）原样保留。
export function accessHost(listen?: string | null): string {
  if (!listen) return '';
  const text = String(listen).trim();
  const m = text.match(/^(.+?):(\d+)$/);
  let host = text;
  let port = '';
  if (m) { host = m[1]; port = m[2]; }
  if (host === '0.0.0.0' || host === '::' || host === '[::]' || host === '') host = '127.0.0.1';
  return port ? `${host}:${port}` : host;
}

/** Opens `url` in the system browser: via the Android bridge, else a new browser tab/window. */
export function openExternal(url: string) {
  if (window.FrameMateAndroid?.openUrl) window.FrameMateAndroid.openUrl(url);
  else window.open(url, "_blank", "noopener");
}

/** onclick for `<a href>`: keeps the href, opens externally. */
export function external(event: MouseEvent) {
  event.preventDefault();
  openExternal((event.currentTarget as HTMLAnchorElement).href);
}

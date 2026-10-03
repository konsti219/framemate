// Web links must not open inside the app's WebView (it would navigate the app away).

/** Opens `url` in the system browser: via the Android bridge, else a new browser tab/window. */
export function openExternal(url: string) {
  if (window.FrameMateAndroid?.openUrl) window.FrameMateAndroid.openUrl(url);
  else window.open(url, "_blank", "noopener");
}

/** Click handler for `<a href="https://…">`: keeps the href for semantics, opens externally. */
export function external(event: MouseEvent) {
  event.preventDefault();
  openExternal((event.currentTarget as HTMLAnchorElement).href);
}

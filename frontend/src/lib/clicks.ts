/**
 * Whether a click is a plain one: the left button, no modifier. Modified
 * clicks (a new tab, a download and so on) are the browser's, so a link's
 * own handler must leave them alone.
 */
export function isPlainClick(event: MouseEvent): boolean {
  return event.button === 0 && !event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey;
}

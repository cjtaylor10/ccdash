/** Match localhost-ish URLs in terminal output and other text.
 *  - http/https
 *  - localhost / 127.0.0.1 / 0.0.0.0 / [::1]
 *  - optional :port
 *  - optional /path (until whitespace or an ANSI escape byte)
 *  We're deliberately conservative — only loopback hosts qualify.
 */
const LOCAL_URL_RE = /https?:\/\/(?:localhost|127\.0\.0\.1|0\.0\.0\.0|\[::1\])(?::\d{1,5})?(?:\/[^\s\x1b]*)?/gi;

/** Strip trailing punctuation common in framework banners
 *  ("Local: http://localhost:3000/.") so the URL is usable as-is. */
function stripTrailingPunct(url: string): string {
  return url.replace(/[.,;:!?)\]}>'"]+$/, '');
}

/** Return the deduplicated, normalized set of loopback URLs found in `text`.
 *  Pure; no side effects. */
export function extractLocalUrls(text: string): string[] {
  const seen = new Set<string>();
  for (const m of text.matchAll(LOCAL_URL_RE)) {
    seen.add(stripTrailingPunct(m[0]));
  }
  return [...seen];
}

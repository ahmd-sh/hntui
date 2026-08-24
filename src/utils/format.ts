export function relativeTime(unixSec?: number): string {
  if (!unixSec) return ""
  const diff = Math.floor(Date.now() / 1000 - unixSec)
  if (diff < 60) return `${diff}s ago`
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  if (diff < 86400 * 30) return `${Math.floor(diff / 86400)}d ago`
  if (diff < 86400 * 365) return `${Math.floor(diff / 2592000)}mo ago`
  return `${Math.floor(diff / 31536000)}y ago`
}

export function hostname(url?: string): string {
  if (!url) return ""
  try {
    return new URL(url).hostname.replace(/^www\./, "")
  } catch {
    return ""
  }
}

const ENTITIES: Record<string, string> = {
  "&amp;": "&",
  "&lt;": "<",
  "&gt;": ">",
  "&quot;": '"',
  "&#x27;": "'",
  "&#39;": "'",
  "&#x2F;": "/",
  "&#47;": "/",
  "&nbsp;": " ",
}

export function decodeEntities(s: string): string {
  return s
    .replace(/&(amp|lt|gt|quot|nbsp|#x27|#39|#x2F|#47);/g, (m) => ENTITIES[m] ?? m)
    .replace(/&#(\d+);/g, (_, n) => String.fromCharCode(parseInt(n, 10)))
    .replace(/&#x([0-9a-f]+);/gi, (_, n) => String.fromCharCode(parseInt(n, 16)))
}

export interface Link {
  text: string
  url: string
}

export function extractLinks(html?: string): Link[] {
  if (!html) return []
  const re = /<a\s+href="([^"]+)"[^>]*>([\s\S]*?)<\/a>/gi
  const out: Link[] = []
  const seen = new Set<string>()
  let m: RegExpExecArray | null
  while ((m = re.exec(html))) {
    const url = decodeEntities(m[1]!)
    if (seen.has(url)) continue
    seen.add(url)
    const text = decodeEntities(m[2]!.replace(/<[^>]+>/g, "")).trim()
    out.push({ text: text || url, url })
  }
  return out
}

// A link into HN itself: /item?id=POSTID or /item?id=POSTID#COMMENTID.
// `id` may be a story OR a comment id
export interface HnItemRef {
  id: number
  anchorId?: number
}

export function parseHnItemLink(url: string): HnItemRef | null {
  try {
    const u = new URL(url)
    const host = u.hostname.replace(/^www\./, "")
    if (host !== "news.ycombinator.com" || u.pathname !== "/item") return null
    const id = Number(u.searchParams.get("id"))
    if (!Number.isInteger(id) || id <= 0) return null
    const anchor = Number(u.hash.slice(1))
    const anchorId =
      u.hash.length > 1 && Number.isInteger(anchor) && anchor > 0 ? anchor : undefined
    return { id, anchorId }
  } catch {
    return null
  }
}

export function htmlToText(html?: string): string {
  if (!html) return ""
  return decodeEntities(
    html
      .replace(/<p>/gi, "\n\n")
      .replace(/<\/p>/gi, "")
      .replace(/<br\s*\/?>/gi, "\n")
      .replace(/<i>(.*?)<\/i>/gi, "$1")
      .replace(/<a\s+href="([^"]+)"[^>]*>(.*?)<\/a>/gi, "$2 ($1)")
      .replace(/<pre><code>([\s\S]*?)<\/code><\/pre>/gi, "\n$1\n")
      .replace(/<code>(.*?)<\/code>/gi, "`$1`")
      .replace(/<[^>]+>/g, "")
  ).trim()
}

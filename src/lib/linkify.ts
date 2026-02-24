/** Escape HTML special characters to prevent XSS. */
export function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;')
}

/**
 * Escape HTML then wrap URLs in clickable <a> tags.
 * Handles trailing punctuation and Wikipedia-style parenthesised URLs.
 */
export function linkify(text: string): string {
  const escaped = escapeHtml(text)
  return escaped.replace(
    /https?:\/\/[^\s<>&"']+/gi,
    (match) => {
      // Strip trailing punctuation that's unlikely part of the URL,
      // but keep balanced parens (for Wikipedia-style URLs).
      let url = match
      let trailing = ''
      const trailingPunct = /[.,;:!?)]+$/
      while (trailingPunct.test(url)) {
        const open = (url.match(/\(/g) || []).length
        const close = (url.match(/\)/g) || []).length
        if (url.endsWith(')') && open < close) {
          trailing = url.slice(-1) + trailing
          url = url.slice(0, -1)
        } else if (url.endsWith(')')) {
          break
        } else {
          trailing = url.slice(-1) + trailing
          url = url.slice(0, -1)
        }
      }
      return `<a href="${url}" target="_blank">${url}</a>${trailing}`
    },
  )
}

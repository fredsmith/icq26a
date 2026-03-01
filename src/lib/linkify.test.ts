import { describe, it, expect } from 'vitest'
import { escapeHtml, linkify } from './linkify'

describe('escapeHtml', () => {
  it('escapes all HTML special characters', () => {
    expect(escapeHtml('&<>"\'')).toBe('&amp;&lt;&gt;&quot;&#039;')
  })

  it('passes through plain text', () => {
    expect(escapeHtml('hello world')).toBe('hello world')
  })

  it('handles empty string', () => {
    expect(escapeHtml('')).toBe('')
  })
})

describe('linkify', () => {
  it('wraps a URL in an anchor tag', () => {
    expect(linkify('visit https://example.com today')).toBe(
      'visit <a href="https://example.com" target="_blank">https://example.com</a> today',
    )
  })

  it('strips trailing punctuation from URLs', () => {
    expect(linkify('see https://example.com.')).toBe(
      'see <a href="https://example.com" target="_blank">https://example.com</a>.',
    )
  })

  it('preserves balanced parens in Wikipedia-style URLs', () => {
    const url = 'https://en.wikipedia.org/wiki/Rust_(programming_language)'
    expect(linkify(url)).toBe(`<a href="${url}" target="_blank">${url}</a>`)
  })

  it('strips trailing paren when unbalanced', () => {
    const input = '(https://example.com)'
    expect(linkify(input)).toBe(
      '(<a href="https://example.com" target="_blank">https://example.com</a>)',
    )
  })

  it('returns plain text unchanged when no URLs', () => {
    expect(linkify('no links here')).toBe('no links here')
  })

  it('escapes HTML in surrounding text', () => {
    expect(linkify('<b>bold</b> https://example.com')).toBe(
      '&lt;b&gt;bold&lt;/b&gt; <a href="https://example.com" target="_blank">https://example.com</a>',
    )
  })
})

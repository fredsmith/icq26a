import { describe, it, expect } from 'vitest'
import { sanitizeLabel } from './windows'

describe('sanitizeLabel', () => {
  it('replaces special characters in Matrix user IDs', () => {
    expect(sanitizeLabel('@alice:matrix.org')).toBe('_alice_matrix_org')
  })

  it('passes through already-clean strings', () => {
    expect(sanitizeLabel('hello_world-123')).toBe('hello_world-123')
  })

  it('handles empty string', () => {
    expect(sanitizeLabel('')).toBe('')
  })

  it('replaces dots and colons', () => {
    expect(sanitizeLabel('room:host.com')).toBe('room_host_com')
  })
})

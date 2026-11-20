import { describe, expect, it } from 'vitest'
import type { ColumnConfig } from '~/types/issue'
import { mergeReorderedColumns } from '~/utils/column-config-helpers'

const column = (id: string): ColumnConfig => ({
  id,
  label: id.toUpperCase(),
  visible: true,
  sortable: true,
})

describe('mergeReorderedColumns', () => {
  it('applies a reorder that moves a column forward', () => {
    const original = [column('a'), column('b'), column('c')]
    const orderedVisible = [column('b'), column('c'), column('a')]

    const result = mergeReorderedColumns(original, orderedVisible, [])

    expect(result?.map(col => col.id)).toEqual(['b', 'c', 'a'])
  })

  it('applies a reorder that moves a column backward', () => {
    const original = [column('a'), column('b'), column('c')]
    const orderedVisible = [column('c'), column('a'), column('b')]

    const result = mergeReorderedColumns(original, orderedVisible, [])

    expect(result?.map(col => col.id)).toEqual(['c', 'a', 'b'])
  })

  it('keeps excluded columns at their original index', () => {
    const original = [column('id'), column('a'), column('b')]
    const orderedVisible = [column('b'), column('a')]

    const result = mergeReorderedColumns(original, orderedVisible, ['id'])

    expect(result?.map(col => col.id)).toEqual(['id', 'b', 'a'])
  })

  it('is the identity when no columns are excluded', () => {
    const original = [column('a'), column('b'), column('c')]
    const orderedVisible = [column('a'), column('b'), column('c')]

    const result = mergeReorderedColumns(original, orderedVisible, [])

    expect(result?.map(col => col.id)).toEqual(['a', 'b', 'c'])
    expect(result).not.toBe(original)
  })

  it('rejects an incomplete reorder instead of dropping columns', () => {
    const original = [column('a'), column('b')]
    const orderedVisible = [column('a')]

    expect(mergeReorderedColumns(original, orderedVisible, [])).toBeNull()
  })
})
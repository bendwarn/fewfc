import { describe, expect, test } from 'bun:test'
import { createSubmissionController } from './submission-controller'

describe('createSubmissionController', () => {
  test('keeps an immediate choice disabled through failure, then permits one explicit retry', async () => {
    let resolveFirst: ((value: boolean) => void) | undefined
    let rejectFirst: ((reason?: unknown) => void) | undefined
    const firstAttempt = new Promise<boolean>((resolve, reject) => {
      resolveFirst = resolve
      rejectFirst = reject
    })
    const submitted: string[] = []
    const controller = createSubmissionController(async (choice: string) => {
      submitted.push(choice)
      return submitted.length === 1 ? await firstAttempt : true
    })

    const first = controller.submit('card-41')
    expect(controller.isSubmitting).toBe(true)
    expect(await controller.submit('card-41')).toBe(false)
    expect(submitted).toEqual(['card-41'])

    rejectFirst?.(new Error('temporary failure'))
    await expect(first).rejects.toThrow('temporary failure')
    expect(controller.isSubmitting).toBe(false)

    const retry = controller.submit('card-41')
    expect(controller.isSubmitting).toBe(true)
    expect(await retry).toBe(true)
    expect(controller.isSubmitting).toBe(false)
    expect(submitted).toEqual(['card-41', 'card-41'])

    resolveFirst?.(true)
  })
})

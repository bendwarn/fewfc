export interface SubmissionController<T> {
  readonly isSubmitting: boolean
  submit(value: T): Promise<boolean>
}

/**
 * Serializes a user-visible command submission. A failed request releases the
 * same local choice for an explicit retry; a second click while it is pending
 * cannot create another command.
 */
export function createSubmissionController<T>(
  submit: (value: T) => Promise<boolean>,
): SubmissionController<T> {
  let submitting = false

  return {
    get isSubmitting() {
      return submitting
    },
    async submit(value) {
      if (submitting) return false
      submitting = true
      try {
        return await submit(value)
      } finally {
        submitting = false
      }
    },
  }
}

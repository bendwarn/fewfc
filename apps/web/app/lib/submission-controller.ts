export interface SubmissionController<T> {
  readonly isSubmitting: boolean
  submit(value: T): Promise<boolean>
}

/**
 * 序列化玩家可見的命令提交。失敗的請求會釋放相同的本機選擇以便明確重試；
 * 等待期間的第二次點擊不能建立另一個命令。
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

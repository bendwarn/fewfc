// 登入故事僅驗證表單呈現與互動，不連線真實帳號服務。
const rejected = async () => ({ error: { message: '帳號或密碼不正確，請重新確認。' } })
export const authClient = {
  signIn: { email: rejected, social: rejected, anonymous: rejected },
  signUp: { email: rejected },
  linkSocial: rejected,
  signOut: async () => ({}),
}

declare module '*.wasm' {
  const module: WebAssembly.Module
  export default module
}

declare module 'cloudflare:workers' {
  export class DurableObject<Env = unknown, State = unknown> {
    protected readonly ctx: {
      storage: {
        get<T>(key: string): Promise<T | undefined>
        put<T>(key: string, value: T): Promise<void>
        delete(key: string): Promise<boolean>
        deleteAll(): Promise<void>
      }
    }
    protected readonly env: Env
    constructor(ctx: unknown, env: Env)
    fetch(request: Request): Promise<Response>
  }
}

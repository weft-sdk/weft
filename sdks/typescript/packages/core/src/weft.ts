import type { Config } from "./config.js"
import type { Contracts } from "./contracts.js"
import { DEFAULTS } from "./defaults.js"
import { WeftError } from "./errors.js"
import { compose, type Context, type Middleware } from "./middleware.js"

/** One way services talk (HTTP, Kafka, ...), run by an adapter on a real library. */
export interface Transport {
  readonly name: string
  /** Connect and begin listening. Called by `weft.start()`. */
  start(): Promise<void>
  /** Stop taking new work. In-flight work carries on. */
  stopIntake(): Promise<void>
  /** Close connections. Called after in-flight work has finished, or the grace period has run out. */
  close(): Promise<void>
}

export interface WeftOptions {
  readonly contracts: Contracts
  readonly config: Config
  /** How long `stop()` waits for in-flight handlers. Default 30 s. */
  readonly shutdownGraceMs?: number
}

export type WeftState = "created" | "starting" | "running" | "stopping" | "stopped"

export interface StopResult {
  /** False when the grace period ran out with handlers still running. */
  readonly drained: boolean
  /** How many handlers were still running when the grace period ran out. */
  readonly abandoned: number
}

const message = (e: unknown): string => (e instanceof Error ? e.message : String(e))

/**
 * The lifecycle shared by every transport: register, `start()`, run handlers through the middleware,
 * `stop()`. Adapters register themselves as transports and send every handler call through `dispatch`.
 */
export class Weft {
  readonly contracts: Contracts
  readonly config: Config
  private readonly graceMs: number
  private readonly transports: Transport[] = []
  private readonly started: Transport[] = []
  private readonly middleware: Middleware<Context>[] = []
  private readonly inflight = new Set<Promise<unknown>>()
  private readonly shutdown = new AbortController()
  private current: WeftState = "created"
  private stopping: Promise<StopResult> | undefined

  constructor(options: WeftOptions) {
    this.contracts = options.contracts
    this.config = options.config
    this.graceMs = options.shutdownGraceMs ?? DEFAULTS.shutdownGraceMs
  }

  get state(): WeftState {
    return this.current
  }

  /** Aborted when `stop()` gives up on handlers that did not finish in the grace period. */
  get shutdownSignal(): AbortSignal {
    return this.shutdown.signal
  }

  /** Adds middleware that runs for every transport, in the order added. */
  use(middleware: Middleware<Context>): this {
    this.middleware.push(middleware)
    return this
  }

  /** Adds a transport. Only before `start()`. */
  register(transport: Transport): this {
    if (this.current !== "created") throw WeftError.internal(`Cannot register ${transport.name}: Weft has already started`)
    if (this.transports.some((t) => t.name === transport.name)) {
      throw WeftError.internal(`The transport ${transport.name} is already registered`)
    }
    this.transports.push(transport)
    return this
  }

  /** Connects every transport, in the order registered. If one fails, the ones already started are closed again. */
  async start(): Promise<void> {
    if (this.current !== "created") throw WeftError.internal(`Cannot start: Weft is ${this.current}`)
    this.current = "starting"
    for (const transport of this.transports) {
      try {
        await transport.start()
        this.started.push(transport)
      } catch (cause) {
        await this.unwind()
        this.current = "stopped"
        throw WeftError.unavailable(`Could not start ${transport.name}: ${message(cause)}`, { cause })
      }
    }
    this.current = "running"
  }

  /**
   * Stops taking new work, waits for in-flight handlers (up to the grace period), then closes every
   * transport. Safe to call more than once. Never throws for a handler that is still running; check
   * the result.
   */
  stop(): Promise<StopResult> {
    if (this.stopping !== undefined) return this.stopping
    if (this.current === "created") {
      this.current = "stopped"
      return Promise.resolve({ drained: true, abandoned: 0 })
    }
    this.current = "stopping"
    this.stopping = this.doStop()
    return this.stopping
  }

  /** Runs `fn` as in-flight work that `stop()` waits for. Refused once stopping has begun. */
  track<T>(fn: () => Promise<T>): Promise<T> {
    if (this.current === "stopping" || this.current === "stopped") {
      return Promise.reject(WeftError.unavailable("Weft is shutting down"))
    }
    const run = fn()
    this.inflight.add(run)
    const done = (): void => {
      this.inflight.delete(run)
    }
    run.then(done, done)
    return run
  }

  /**
   * Runs `handler` for one call: the global middleware, then `extra` (the transport's own), then
   * the handler, counted as in-flight work.
   */
  dispatch<C extends Context>(
    ctx: C,
    handler: (ctx: C) => Promise<void> | void,
    extra: readonly Middleware<C>[] = [],
  ): Promise<void> {
    const chain = compose<C>([...(this.middleware as Middleware<Context>[] as Middleware<C>[]), ...extra])
    return this.track(() => chain(ctx, handler))
  }

  private async doStop(): Promise<StopResult> {
    const reversed = [...this.started].reverse()
    await Promise.allSettled(reversed.map((t) => t.stopIntake()))
    const result = await this.drain()
    if (!result.drained) this.shutdown.abort()
    for (const transport of reversed) {
      try {
        await transport.close()
      } catch {
        // A transport that fails to close must not keep the others open.
      }
    }
    this.current = "stopped"
    return result
  }

  private async drain(): Promise<StopResult> {
    const deadline = Date.now() + this.graceMs
    while (this.inflight.size > 0) {
      const left = deadline - Date.now()
      if (left <= 0) return { drained: false, abandoned: this.inflight.size }
      let timer: ReturnType<typeof setTimeout> | undefined
      const timeout = new Promise<void>((resolve) => {
        timer = setTimeout(resolve, left)
      })
      await Promise.race([Promise.allSettled([...this.inflight]), timeout])
      clearTimeout(timer)
    }
    return { drained: true, abandoned: 0 }
  }

  private async unwind(): Promise<void> {
    for (const transport of [...this.started].reverse()) {
      await transport.stopIntake().catch(() => undefined)
      await transport.close().catch(() => undefined)
    }
    this.started.length = 0
  }
}

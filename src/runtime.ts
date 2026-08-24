import { Layer, ManagedRuntime } from "effect"
import { HnApiLive, HnTransportLive } from "./api/hn"
import { HnTransportLinkDemo } from "./api/hnDemo"

// The app's single composition point: the whole layer graph is decided here,
// once, at the edge. Everything below runs against whatever this provides —
// HN_DEMO=1 swaps the transport and no other file knows or cares.
// (A real app would keep demo code out of the production bundle via separate
// entry points; for a TUI this trade is fine.)
const transport = process.env.HN_DEMO === "1" ? HnTransportLinkDemo : HnTransportLive

export const AppRuntime = ManagedRuntime.make(HnApiLive.pipe(Layer.provide(transport)))

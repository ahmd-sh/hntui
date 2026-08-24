import { Match } from "effect"
import type { HnError } from "../api/hn"

// The last mile of typed errors: every HnError member must map to copy a
// human can act on. Match.exhaustive makes this a compile-time guarantee —
// adding a new error to HnError breaks this build until it gets a message.
export const hnErrorMessage: (e: HnError) => string = Match.type<HnError>().pipe(
  Match.tag("HnRequestError", () => "Couldn't reach Hacker News — check your connection."),
  Match.tag("HnTimeoutError", () => "Hacker News took too long to respond."),
  Match.tag("HnStatusError", (e) => `Hacker News returned HTTP ${e.status}.`),
  Match.tag("HnDecodeError", () => "Hacker News sent a response the app couldn't understand."),
  Match.exhaustive,
)

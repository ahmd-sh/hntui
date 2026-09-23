#!/usr/bin/env bun
import { createCliRenderer } from "@opentui/core"
import { createRoot } from "@opentui/react"
import "opentui-spinner/react"
import { App } from "./App"
import { VERSION } from "./version"
import { selfUpdate } from "./utils/selfUpdate"

const arg = process.argv[2]
if (arg === "--version" || arg === "-v" || arg === "version") {
  console.log(`hntui ${VERSION}`)
  process.exit(0)
} else if (arg === "update") {
  process.exit(await selfUpdate())
} else if (arg) {
  console.error(`hntui: unknown command "${arg}"\nusage: hntui [update | --version]`)
  process.exit(1)
} else {
  const renderer = await createCliRenderer({ useMouse: true })
  createRoot(renderer).render(<App />)
}

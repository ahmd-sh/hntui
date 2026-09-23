import pkg from "../package.json"

// Inlined at build time — compiled binaries carry their version with them.
export const VERSION: string = pkg.version

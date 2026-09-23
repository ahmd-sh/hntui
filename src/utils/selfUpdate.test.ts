import { describe, expect, test } from "bun:test"
import { compareVersions, releaseAsset } from "./selfUpdate"

describe("compareVersions", () => {
  test("orders plain versions", () => {
    expect(compareVersions("0.4.0", "0.3.0")).toBeGreaterThan(0)
    expect(compareVersions("0.3.0", "0.4.0")).toBeLessThan(0)
    expect(compareVersions("0.4.0", "0.4.0")).toBe(0)
  })

  test("compares numerically, not lexically", () => {
    expect(compareVersions("0.10.0", "0.9.0")).toBeGreaterThan(0)
    expect(compareVersions("1.0.0", "0.99.99")).toBeGreaterThan(0)
  })

  test("tolerates missing segments", () => {
    expect(compareVersions("1.0", "1.0.0")).toBe(0)
    expect(compareVersions("1", "1.0.1")).toBeLessThan(0)
  })
})

describe("releaseAsset", () => {
  test("maps supported platforms", () => {
    expect(releaseAsset("darwin", "arm64")).toBe("hntui-darwin-arm64.tar.gz")
    expect(releaseAsset("linux", "x64")).toBe("hntui-linux-x64.tar.gz")
    expect(releaseAsset("linux", "arm64")).toBe("hntui-linux-arm64.tar.gz")
  })

  test("unsupported platforms get null (Intel macOS, windows)", () => {
    expect(releaseAsset("darwin", "x64")).toBeNull()
    expect(releaseAsset("win32", "x64")).toBeNull()
  })
})

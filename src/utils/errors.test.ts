import { describe, expect, test } from "bun:test"
import { HnDecodeError, HnRequestError, HnStatusError, HnTimeoutError } from "../api/hn"
import { hnErrorMessage } from "./errors"

describe("hnErrorMessage", () => {
  test("network failure suggests checking the connection", () => {
    expect(hnErrorMessage(new HnRequestError({ path: "x", cause: 1 }))).toMatch(/connection/)
  })

  test("timeout says so", () => {
    expect(hnErrorMessage(new HnTimeoutError({ path: "x" }))).toMatch(/too long/)
  })

  test("status errors surface the HTTP code", () => {
    expect(hnErrorMessage(new HnStatusError({ path: "x", status: 503 }))).toContain("503")
  })

  test("decode errors are covered", () => {
    expect(hnErrorMessage(new HnDecodeError({ path: "x", issue: "bad" }))).toMatch(/understand/)
  })
})

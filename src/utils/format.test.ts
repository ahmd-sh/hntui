import { describe, expect, test } from "bun:test"
import { parseHnItemLink } from "./format"

describe("parseHnItemLink", () => {
  test("story link", () => {
    expect(parseHnItemLink("https://news.ycombinator.com/item?id=8863")).toEqual({
      id: 8863,
      anchorId: undefined,
    })
  })

  test("story link with comment anchor", () => {
    expect(parseHnItemLink("https://news.ycombinator.com/item?id=8863#8917")).toEqual({
      id: 8863,
      anchorId: 8917,
    })
  })

  test("www and http variants", () => {
    expect(parseHnItemLink("http://www.news.ycombinator.com/item?id=1")).toEqual({
      id: 1,
      anchorId: undefined,
    })
  })

  test("extra query params are tolerated", () => {
    expect(parseHnItemLink("https://news.ycombinator.com/item?id=42&p=2")?.id).toBe(42)
  })

  test("non-item HN pages are external", () => {
    expect(parseHnItemLink("https://news.ycombinator.com/user?id=pg")).toBeNull()
    expect(parseHnItemLink("https://news.ycombinator.com/newest")).toBeNull()
  })

  test("other hosts are external", () => {
    expect(parseHnItemLink("https://example.com/item?id=1")).toBeNull()
    expect(parseHnItemLink("https://hn.algolia.com/item?id=1")).toBeNull()
  })

  test("missing or malformed ids are external", () => {
    expect(parseHnItemLink("https://news.ycombinator.com/item")).toBeNull()
    expect(parseHnItemLink("https://news.ycombinator.com/item?id=abc")).toBeNull()
    expect(parseHnItemLink("https://news.ycombinator.com/item?id=-5")).toBeNull()
  })

  test("non-numeric anchor is ignored, link still internal", () => {
    expect(parseHnItemLink("https://news.ycombinator.com/item?id=7#up_8")).toEqual({
      id: 7,
      anchorId: undefined,
    })
  })

  test("garbage is external", () => {
    expect(parseHnItemLink("not a url")).toBeNull()
  })
})

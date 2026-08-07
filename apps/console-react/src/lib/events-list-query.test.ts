import assert from "node:assert/strict";
import test from "node:test";
import { buildEventsListQuery } from "./events-list-query.js";

test("maps page/pageSize to limit and offset for server pagination", () => {
  const q = buildEventsListQuery({ page: 3, pageSize: 20 });
  assert.equal(q.get("limit"), "20");
  assert.equal(q.get("offset"), "40");
  assert.equal(q.get("layer"), "l7");
  assert.equal(q.get("triage"), "true");
});

test("starts at offset 0 on first page", () => {
  const q = buildEventsListQuery({ page: 1, pageSize: 10 });
  assert.equal(q.get("limit"), "10");
  assert.equal(q.get("offset"), "0");
});

test("includes optional category and site_id filters", () => {
  const q = buildEventsListQuery({
    page: 1,
    pageSize: 50,
    category: "sqli",
    siteId: "waf-test",
  });
  assert.equal(q.get("category"), "sqli");
  assert.equal(q.get("site_id"), "waf-test");
});

test("omits empty filters", () => {
  const q = buildEventsListQuery({ page: 1, pageSize: 10, category: "", siteId: "" });
  assert.equal(q.has("category"), false);
  assert.equal(q.has("site_id"), false);
});

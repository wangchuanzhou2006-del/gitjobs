import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";

const source = await readFile(
  new URL("../../gitjobs-server/static/js/dashboard/moderator/talent-review.js", import.meta.url),
  "utf8",
);

function consumer() {
  const listeners = new Map();
  const calls = [];
  const context = {
    document: {
      addEventListener(name, listener) {
        listeners.set(name, listener);
      },
    },
    window: {
      location: {
        assign(path) {
          calls.push(["assign", path]);
        },
        reload() {
          calls.push(["reload"]);
        },
      },
    },
  };
  vm.runInNewContext(source, context);
  return { calls, listeners };
}

test("detail refresh events reload the current workbench", () => {
  const { calls, listeners } = consumer();

  listeners.get("talent-review-refresh")({
    detail: { destination: "detail" },
  });

  assert.deepEqual(calls, [["reload"]]);
});

test("terminal refresh events navigate to the privacy-safe queue path", () => {
  const { calls, listeners } = consumer();

  listeners.get("talent-review-refresh")({
    detail: { destination: "queue" },
  });

  assert.deepEqual(calls, [["assign", "/dashboard/moderator/talent/review"]]);
});

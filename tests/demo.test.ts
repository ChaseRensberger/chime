import { afterEach, describe, expect, test } from "bun:test";
import { createDemoEngine, SAMPLE_TRANSCRIPT } from "../src/engine/demo";
import type { DemoEngine } from "../src/engine/types";

const engines: DemoEngine[] = [];
function setup() {
  const engine = createDemoEngine(5);
  engines.push(engine);
  return engine;
}
afterEach(() => engines.splice(0).forEach((engine) => engine.dispose()));
const settle = () => Bun.sleep(20);

describe("demo session ownership", () => {
  test("a cancelled completion cannot overwrite a new recording", async () => {
    const engine = setup();
    engine.start();
    engine.finish();
    engine.cancel();
    engine.start();
    await settle();
    expect(engine.getSnapshot().phase).toBe("recording");
    expect(engine.getSnapshot().lastResult).toBeNull();
  });

  test("silence and failure preserve the last result, and a fresh session recovers", async () => {
    const engine = setup();
    engine.start();
    engine.finish();
    await settle();
    const result = engine.getSnapshot().lastResult;
    expect(result?.text).toBe(SAMPLE_TRANSCRIPT);
    for (const outcome of ["silence", "error"] as const) {
      engine.setOutcome(outcome);
      engine.start();
      engine.finish();
      await settle();
      expect(engine.getSnapshot().lastResult).toBe(result);
      expect(engine.getSnapshot().phase).toBe(outcome === "error" ? "error" : "idle");
    }
    engine.setOutcome("success");
    engine.start();
    engine.finish();
    await settle();
    expect(engine.getSnapshot().phase).toBe("complete");
  });

  test("duplicate finish and start during processing cannot create another session", async () => {
    const engine = setup();
    engine.start();
    engine.finish();
    engine.finish();
    engine.start();
    expect(engine.getSnapshot().phase).toBe("processing");
    await settle();
    expect(engine.getSnapshot().lastResult?.text).toBe(SAMPLE_TRANSCRIPT);
  });

  test("disposing removes the last transcript and prevents late notifications", async () => {
    const engine = setup();
    engine.start();
    engine.finish();
    let notifications = 0;
    engine.subscribe(() => notifications++);
    engine.dispose();
    await settle();
    expect(notifications).toBe(0);
    expect(engine.getSnapshot().lastResult).toBeNull();
  });
});

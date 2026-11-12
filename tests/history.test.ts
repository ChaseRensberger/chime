import { expect, test } from "bun:test";
import { createDemoEngine } from "../src/engine/demo";
import { createHistory } from "../src/engine/history";

test("history records successes, survives dismissing the last result, and respects Off", async () => {
  const engine = createDemoEngine(1);
  const history = createHistory(engine);
  try {
    engine.start();
    engine.finish();
    await Bun.sleep(20);
    engine.clearResult();
    expect((await history.list("")).total).toBe(1);
    engine.setOutcome("error");
    engine.start();
    engine.finish();
    await Bun.sleep(20);
    expect((await history.list("")).total).toBe(1);
    await history.setRetention("off");
    engine.setOutcome("success");
    engine.start();
    engine.finish();
    await Bun.sleep(20);
    expect((await history.list("")).total).toBe(1);
    await history.clear();
    expect((await history.list("")).total).toBe(0);
  } finally {
    history.dispose();
    engine.dispose();
  }
});

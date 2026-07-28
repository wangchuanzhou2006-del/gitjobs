import assert from "node:assert/strict";
import test from "node:test";

import { startTalentAiMock } from "../e2e/shared/talent-ai-mock.mjs";

const completionRequest = (fragment) => ({
  model: "e2e-deterministic-v1",
  messages: [
    { role: "system", content: "talent-extract-v1" },
    { role: "user", content: fragment },
  ],
  temperature: 0,
  response_format: { type: "json_object" },
});

test("deterministic E2E provider returns a complete extraction through the OpenAI-compatible boundary", async () => {
  const service = await startTalentAiMock({ host: "127.0.0.1", port: 0 });
  try {
    const response = await fetch(`${service.baseUrl}/v1/chat/completions`, {
      method: "POST",
      headers: {
        authorization: "Bearer deliberately-invalid-e2e-token",
        "content-type": "application/json",
      },
      body: JSON.stringify(completionRequest("E2E_SAFE_HIRING_CHROMIUM")),
    });

    assert.equal(response.status, 200);
    const completion = await response.json();
    const extraction = JSON.parse(completion.choices[0].message.content);
    assert.equal(extraction.kind, "hiring");
    assert.equal(extraction.title, "E2E 安全招聘 · chromium");
    assert.equal(extraction.confidence, 0.98);
    assert.deepEqual(extraction.missingFields, []);
    assert.equal(extraction.contacts[0].value, "13800000001");
  } finally {
    await service.close();
  }
});

test("deterministic E2E provider rejects unknown fixtures without reflecting protected input", async () => {
  const service = await startTalentAiMock({ host: "127.0.0.1", port: 0 });
  try {
    const protectedInput = "UNKNOWN_FIXTURE reserved_private_value_1201";
    const response = await fetch(`${service.baseUrl}/v1/chat/completions`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(completionRequest(protectedInput)),
    });

    assert.equal(response.status, 422);
    assert.equal((await response.text()).includes(protectedInput), false);
  } finally {
    await service.close();
  }
});

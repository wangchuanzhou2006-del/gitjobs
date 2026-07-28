import type { FullConfig } from "@playwright/test";

import { startTalentAiMock } from "./talent-ai-mock.mjs";

export default async function globalSetup(_config: FullConfig) {
  const configuredPort = process.env.GITJOBS_E2E_TALENT_AI_PORT;
  if (!configuredPort) return;
  const port = Number.parseInt(configuredPort, 10);
  if (!Number.isInteger(port) || port < 1 || port > 65535)
    throw new Error("GITJOBS_E2E_TALENT_AI_PORT must be a valid TCP port");

  const service = await startTalentAiMock({ host: "127.0.0.1", port });
  return async () => {
    await service.close();
  };
}

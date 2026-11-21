import type { FullConfig } from "@playwright/test";

import { startTalentAiMock } from "./talent-ai-mock.mjs";

export default async function globalSetup(_config: FullConfig) {
  const configuredPort = process.env.GITJOBS_E2E_TALENT_AI_PORT;
  const services: { close: () => Promise<void> }[] = [];
  const parsePort = (value: string, name: string) => {
    const port = Number.parseInt(value, 10);
    if (!Number.isInteger(port) || port < 1 || port > 65535)
      throw new Error(`${name} must be a valid TCP port`);
    return port;
  };
  if (configuredPort) {
    services.push(await startTalentAiMock({
      host: "127.0.0.1",
      port: parsePort(configuredPort, "GITJOBS_E2E_TALENT_AI_PORT"),
    }));
  }
  return async () => {
    await Promise.all(services.map((service) => service.close()));
  };
}

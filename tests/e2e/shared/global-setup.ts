import type { FullConfig } from "@playwright/test";

import { startTalentAiMock } from "./talent-ai-mock.mjs";
import { startTenderAnalysisMock } from "./tender-analysis-mock.mjs";

export default async function globalSetup(_config: FullConfig) {
  const configuredPort = process.env.GITJOBS_E2E_TALENT_AI_PORT;
  const tenderPort = process.env.GITJOBS_E2E_TENDER_PORT;
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
  if (tenderPort) {
    const token = process.env.GITJOBS_E2E_TENDER_TOKEN;
    if (!token || token.length < 32)
      throw new Error("GITJOBS_E2E_TENDER_TOKEN must be set for tender acceptance");
    services.push(await startTenderAnalysisMock({
      host: "127.0.0.1",
      port: parsePort(tenderPort, "GITJOBS_E2E_TENDER_PORT"),
      token,
    }));
  }
  return async () => {
    await Promise.all(services.map((service) => service.close()));
  };
}

import { createServer } from "node:http";

const BUDGET = "Budget amount: CNY 2800000";
const DEADLINE = "Bid deadline: 2026-09-01";
const MAX_BYTES = 64 * 1024 * 1024;

async function readBounded(request) {
  const chunks = [];
  let size = 0;
  for await (const chunk of request) {
    size += chunk.length;
    if (size > MAX_BYTES) throw new Error("request_too_large");
    chunks.push(chunk);
  }
  return Buffer.concat(chunks);
}

const citation = (page, quote) => ({
  page_number: page,
  chunk_id: `page-${page}-chunk-0`,
  quote,
});

const missing = () => ({ value: null, confidence: 0, requires_review: true, citations: [] });
const confirmed = (value, page, quote) => ({
  value,
  confidence: 0.95,
  requires_review: false,
  citations: [citation(page, quote)],
});

function parsedDocument(forged) {
  const second = forged ? `${DEADLINE}\nE2E_FORGED_BID` : DEADLINE;
  return {
    page_count: 2,
    pages: [
      { page_number: 1, text: BUDGET, chunks: [{ chunk_id: "page-1-chunk-0", page_number: 1, text: BUDGET }] },
      { page_number: 2, text: second, chunks: [{ chunk_id: "page-2-chunk-0", page_number: 2, text: second }] },
    ],
  };
}

function report(document) {
  const forged = document.pages[1].text.includes("E2E_FORGED_BID");
  return {
    schema_version: "tender-analysis.v1",
    project_info: {
      project_name: missing(),
      procurement_entity: missing(),
      budget_amount: confirmed("CNY 2800000", 1, forged ? "Invented budget: CNY 9999999" : BUDGET),
      budget_currency: confirmed("CNY", 1, BUDGET),
      service_period: missing(),
      bid_deadline: confirmed("2026-09-01", 2, DEADLINE),
    },
    qualifications: [], risks: [], required_materials: [],
    unknowns: ["project_name", "procurement_entity", "service_period"].map((field_name) => ({
      field_name, reason: "missing", detail: "Not present in synthetic document", citations: [],
    })),
    analysis_meta: {
      model_provider: "e2e", model_name: "fixture-model",
      prompt_versions: {
        project_info: "tender-project-info-v1", qualifications: "tender-qualifications-v1",
        risks: "tender-risks-v1", materials: "tender-materials-v1", review: "tender-review-v1",
      },
      contract_version: "tender-analysis.v1", completeness: "partial",
    },
  };
}

export async function startTenderAnalysisMock({ host, port, token }) {
  const server = createServer(async (request, response) => {
    const json = (status, value) => {
      response.writeHead(status, { "content-type": "application/json", "cache-control": "no-store" });
      response.end(JSON.stringify(value));
    };
    if (request.headers.authorization !== `Bearer ${token}`) {
      json(401, { detail: "invalid internal credentials" });
      return;
    }
    if (request.method !== "POST") {
      json(404, { detail: "not_found" });
      return;
    }
    try {
      const data = await readBounded(request);
      if (request.url === "/internal/v1/parse") {
        const source = data.toString("latin1");
        if (!source.includes("%PDF-") || !source.includes(BUDGET) || !source.includes(DEADLINE)) {
          json(422, { status: "failed", parsed_document: null, error_code: "invalid_pdf" });
          return;
        }
        json(200, { status: "completed", parsed_document: parsedDocument(source.includes("E2E_FORGED_BID")), error_code: null });
        return;
      }
      if (request.url === "/internal/v1/analyze") {
        const input = JSON.parse(data.toString("utf8"));
        if (input?.document?.page_count !== 2) throw new Error("invalid_request");
        json(200, report(input.document));
        return;
      }
      json(404, { detail: "not_found" });
    } catch {
      json(400, { detail: "invalid_request" });
    }
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, host, resolve);
  });
  const address = server.address();
  return {
    baseUrl: `http://${host}:${address.port}`,
    close: () => new Promise((resolve) => server.close(resolve)),
  };
}

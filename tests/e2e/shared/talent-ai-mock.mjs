import { createServer } from "node:http";

const MAX_REQUEST_BYTES = 64 * 1024;

const projectSuffix = (fragment) => {
  for (const suffix of ["CHROMIUM", "FIREFOX", "WEBKIT"]) {
    if (fragment.includes(suffix)) return suffix.toLowerCase();
  }
  return "unit";
};

const contactFor = (fragment, offset = 0) => {
  const base = {
    chromium: 1,
    firefox: 11,
    webkit: 21,
    unit: 1,
  }[projectSuffix(fragment)];
  return `1380000${String(base + offset).padStart(4, "0")}`;
};

const profileFor = (fragment, variant, role) => {
  const profiles = {
    safe: {
      chromium: { region: "上海", city: "上海", district: "浦东新区", salaryMin: 7000, salaryMax: 9000 },
      firefox: { region: "浙江", city: "杭州", district: "西湖区", salaryMin: 10000, salaryMax: 12000 },
      webkit: { region: "四川", city: "成都", district: "锦江区", salaryMin: 13000, salaryMax: 15000 },
      unit: { region: "北京", city: "北京", district: "朝阳区", salaryMin: 7000, salaryMax: 9000 },
    },
    medium: {
      chromium: { region: "北京", city: "北京", district: "海淀区", salaryMin: 20000, salaryMax: 22000 },
      firefox: { region: "广东", city: "广州", district: "天河区", salaryMin: 23000, salaryMax: 25000 },
      webkit: { region: "江苏", city: "南京", district: "鼓楼区", salaryMin: 26000, salaryMax: 28000 },
      unit: { region: "北京", city: "北京", district: "海淀区", salaryMin: 20000, salaryMax: 22000 },
    },
    high: {
      chromium: { region: "天津", city: "天津", district: "和平区", salaryMin: 30000, salaryMax: 32000 },
      firefox: { region: "广东", city: "深圳", district: "南山区", salaryMin: 33000, salaryMax: 35000 },
      webkit: { region: "江苏", city: "苏州", district: "姑苏区", salaryMin: 36000, salaryMax: 38000 },
      unit: { region: "天津", city: "天津", district: "和平区", salaryMin: 30000, salaryMax: 32000 },
    },
    new: {
      chromium: { region: "陕西", city: "西安", district: "雁塔区", salaryMin: 40000, salaryMax: 42000 },
      firefox: { region: "重庆", city: "重庆", district: "渝中区", salaryMin: 43000, salaryMax: 45000 },
      webkit: { region: "山东", city: "青岛", district: "市南区", salaryMin: 46000, salaryMax: 48000 },
      unit: { region: "陕西", city: "西安", district: "雁塔区", salaryMin: 40000, salaryMax: 42000 },
    },
    paused: {
      chromium: { region: "湖北", city: "武汉", district: "江汉区", salaryMin: 50000, salaryMax: 52000 },
      firefox: { region: "湖南", city: "长沙", district: "天心区", salaryMin: 53000, salaryMax: 55000 },
      webkit: { region: "福建", city: "厦门", district: "思明区", salaryMin: 56000, salaryMax: 58000 },
      unit: { region: "湖北", city: "武汉", district: "江汉区", salaryMin: 50000, salaryMax: 52000 },
    },
  };
  const profile = profiles[variant][projectSuffix(fragment)];
  return { ...profile, roles: [role], skills: [role, "食品安全"] };
};

const completeExtraction = (fragment, overrides = {}) => ({
  kind: "hiring",
  confidence: 0.98,
  title: "E2E 安全招聘",
  scene: "E2E 餐饮门店",
  region: "北京",
  city: "北京",
  district: "朝阳区",
  salaryMin: 7000,
  salaryMax: 9000,
  roles: ["切配"],
  skills: ["切配", "食品安全"],
  experienceYears: 3,
  hoursPerDay: 8,
  restDaysPerMonth: 4,
  lodging: "提供",
  housingAllowance: 0,
  availability: "随时到岗",
  urgency: "常规",
  publishedAt: "2026-07-28T00:00:00Z",
  contacts: [{ contact_type: "phone", value: contactFor(fragment) }],
  missingFields: [],
  riskCandidates: [],
  ...overrides,
});

const extractionFor = (fragment) => {
  if (fragment.includes("E2E_SAFE_HIRING")) {
    return completeExtraction(fragment, {
      ...profileFor(fragment, "safe", "烘焙"),
      title: `E2E 安全招聘 · ${projectSuffix(fragment)}`,
    });
  }
  if (fragment.includes("E2E_SAFE_SEEKING")) {
    return completeExtraction(fragment, {
      ...profileFor(fragment, "safe", "面点"),
      kind: "seeking",
      title: `E2E 安全求职 · ${projectSuffix(fragment)}`,
      contacts: [{ contact_type: "wechat", value: `test_candidate_${projectSuffix(fragment)}_02` }],
    });
  }
  if (fragment.includes("E2E_MEDIUM_FEE")) {
    return completeExtraction(fragment, {
      ...profileFor(fragment, "medium", "服务员"),
      confidence: 0.85,
      title: `E2E 费用待核招聘 · ${projectSuffix(fragment)}`,
      contacts: [{ contact_type: "phone", value: contactFor(fragment, 2) }],
      missingFields: ["fee_policy_review"],
    });
  }
  if (fragment.includes("E2E_HIGH_RISK")) {
    return completeExtraction(fragment, {
      ...profileFor(fragment, "high", "店长"),
      title: `E2E 高风险招聘 · ${projectSuffix(fragment)}`,
      contacts: [{ contact_type: "phone", value: contactFor(fragment, 3) }],
    });
  }
  if (fragment.includes("E2E_DUPLICATE")) {
    return completeExtraction(fragment, {
      title: `E2E 重复切配招聘 · ${projectSuffix(fragment)}`,
      region: "北京",
      city: "北京",
      district: "朝阳区",
      salaryMin: 7000,
      salaryMax: 9000,
      roles: ["切配"],
      skills: ["切配", "食品安全"],
      contacts: [{ contact_type: "phone", value: contactFor(fragment, 4) }],
    });
  }
  if (fragment.includes("E2E_NEW_SOURCE")) {
    return completeExtraction(fragment, {
      ...profileFor(fragment, "new", "洗碗"),
      title: `E2E 新来源招聘 · ${projectSuffix(fragment)}`,
      contacts: [{ contact_type: "phone", value: contactFor(fragment, 5) }],
    });
  }
  if (fragment.includes("E2E_PAUSED_SAFE")) {
    return completeExtraction(fragment, {
      ...profileFor(fragment, "paused", "烧烤"),
      title: `E2E 暂停后安全招聘 · ${projectSuffix(fragment)}`,
      contacts: [{ contact_type: "phone", value: contactFor(fragment, 6) }],
    });
  }
  return null;
};

const readJson = async (request) => {
  const chunks = [];
  let size = 0;
  for await (const chunk of request) {
    size += chunk.length;
    if (size > MAX_REQUEST_BYTES) throw new Error("request_too_large");
    chunks.push(chunk);
  }
  return JSON.parse(Buffer.concat(chunks).toString("utf8"));
};

export async function startTalentAiMock({ host, port }) {
  const server = createServer(async (request, response) => {
    const json = (status, body) => {
      response.writeHead(status, {
        "cache-control": "no-store",
        "content-type": "application/json; charset=utf-8",
      });
      response.end(JSON.stringify(body));
    };
    if (request.method !== "POST" || request.url !== "/v1/chat/completions") {
      json(404, { error: "not_found" });
      return;
    }
    try {
      const payload = await readJson(request);
      const fragment = payload?.messages?.find((message) => message?.role === "user")?.content;
      const extraction = typeof fragment === "string" ? extractionFor(fragment) : null;
      if (!extraction) {
        json(422, { error: "unknown_e2e_fixture" });
        return;
      }
      json(200, {
        choices: [{ message: { content: JSON.stringify(extraction) } }],
      });
    } catch {
      json(400, { error: "invalid_request" });
    }
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, host, resolve);
  });
  const address = server.address();
  return {
    baseUrl: `http://${host}:${address.port}`,
    close: () => new Promise((resolve, reject) => server.close((error) => (error ? reject(error) : resolve()))),
  };
}

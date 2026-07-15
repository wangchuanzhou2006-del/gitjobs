# GitJobs Foundation Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在不修改业务功能的前提下，在当前Windows电脑上建立可重复的GitJobs开发环境，并证明Rust、数据库迁移、数据库测试和端到端测试能够通过。

**Architecture:** Windows负责Codex工作区和Docker Desktop，WSL2 Ubuntu 24.04负责Rust编译、迁移脚本及Playwright测试。PostgreSQL运行在Docker容器中，测试配置保存在被Git忽略的`config/local`目录；仓库只提交不含真实秘密的示例配置。

**Tech Stack:** WSL2 Ubuntu 24.04、Rust 1.96.1、PostgreSQL 16加pgTAP（数据库测试）、PostGIS 17-3.5（端到端测试）、tern 2.3.2、Tailwind CSS 4.0.17、Node.js 20、Playwright 1.61.1、wait-on 9.0.10。

## Global Constraints

- 本计划只验证CNCF GitJobs底座，不修改`gitjobs-server/src`、`gitjobs-server/templates`、`database/migrations`中的业务实现。
- 所有Rust命令必须使用1.96.1工具链，与`Cargo.toml`和CI一致。
- 所有Cargo构建使用仓库现有`Cargo.lock`；不得执行`cargo update`。
- 数据库测试使用仓库`database/tests/Dockerfile-postgres-db-tests`定义的PostgreSQL 16；端到端测试使用与`.github/workflows/e2e.yml`一致的`postgis/postgis:17-3.5`。
- 本地数据库密码只允许存在于`config/local`；该目录必须被Git忽略。
- GitJobs采用Apache-2.0并要求DCO。执行任何提交前，用户必须提供真实Git姓名和邮箱；所有提交使用`git commit -s`。
- 当前仓库已有暂存的设计文档。开始执行前先单独提交该文档，避免后续环境提交混入设计稿。
- WSL中的仓库路径固定为`/mnt/c/Users/25351/gitjobs-candidate`。
- 阶段0任一基线测试失败时，保留完整错误输出并停止对应任务；不在本计划内顺手修改业务代码。

---

## Plan Boundaries

完整产品包含多个可独立验收的子系统。本文件只执行阶段0，后续严格按以下顺序分别编写、审批和执行独立计划：

1. `2026-07-13-shared-platform-shell.md`：中文化、双入口导航、手机号登录和共享权限。
2. `2026-07-13-project-intelligence.md`：食堂项目模型、列表、筛选、可信度、收藏和联系闭环。
3. `2026-07-13-talent-marketplace.md`：招聘、求职、发布、有效期、联系方式保护和后台处理。
4. `2026-07-13-ai-ingestion-review.md`：公开来源采集、AI解析、去重、规则和分级审核。
5. `2026-07-13-matching-notifications.md`：项目匹配、人才双向匹配、订阅和通知。

每个后续计划只在前一阶段通过验收后锁定具体文件和接口，避免底座验证发现差异后产生大面积失效计划。

## File Map

**Create**

- `config/examples/server.local.yml`：不含真实秘密的本地服务器配置示例。
- `config/examples/tern.tests.local.conf`：数据库测试迁移配置示例。
- `config/examples/tern.e2e.local.conf`：端到端数据库迁移配置示例。

**Modify**

- `.gitignore`：忽略本地配置、Playwright报告和临时测试产物。

**Generated but not committed**

- `config/local/server.yml`
- `config/local/tern.tests.conf`
- `config/local/tern.e2e.conf`
- `package.json`
- `package-lock.json`
- `node_modules/`
- `playwright-report/`
- `test-results/`

---

### Task 1: Establish the Windows/WSL development runtime

**Files:** None.

**Interfaces:**

- Consumes: Windows 11、管理员权限、当前仓库路径。
- Produces: 可运行Ubuntu 24.04、Docker、Rust、Node、tern、Tailwind CSS和pg_prove的开发终端。

- [ ] **Step 1: Confirm Git identity is available**

执行前请用户提供真实姓名和邮箱。只在当前仓库配置，不修改全局Git设置：

```powershell
git config user.name "用户提供的真实姓名"
git config user.email "用户提供的真实邮箱"
git config --get user.name
git config --get user.email
```

Expected: 最后两行与用户提供的信息完全一致。未获得真实信息时停止执行，不使用Codex或虚构身份代替。

- [ ] **Step 2: Commit the already approved design document separately**

```powershell
git commit -s -m "docs: add AI catering platform design"
```

Expected: 提交只包含`docs/superpowers/specs/2026-07-13-ai-catering-intelligence-platform-design.md`。运行`git show --stat --oneline HEAD`确认没有业务文件。

- [ ] **Step 3: Install WSL2 Ubuntu 24.04**

在管理员PowerShell中运行：

```powershell
wsl --install -d Ubuntu-24.04
```

Expected: Windows报告Ubuntu已安装；如果系统要求重启，重启后重新打开本任务。

验证：

```powershell
wsl --list --verbose
```

Expected: 列表包含`Ubuntu-24.04`且`VERSION`为`2`。

- [ ] **Step 4: Install Docker Desktop and enable WSL integration**

在管理员PowerShell中运行：

```powershell
winget install -e --id Docker.DockerDesktop
```

启动Docker Desktop，在`Settings > Resources > WSL Integration`中启用`Ubuntu-24.04`，然后在Ubuntu终端运行：

```bash
docker version
docker run --rm hello-world
```

Expected: `docker version`同时显示Client和Server；第二条命令输出`Hello from Docker!`。

- [ ] **Step 5: Install Linux build prerequisites**

在Ubuntu终端运行：

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev curl ca-certificates git postgresql-client perl cpanminus
sudo cpanm --notest TAP::Parser::SourceHandler::pgTAP
```

Expected: `gcc --version`、`psql --version`和`pg_prove --version`均成功退出。

- [ ] **Step 6: Install the pinned Rust toolchain**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
rustup toolchain install 1.96.1 --component clippy --component rustfmt
cd /mnt/c/Users/25351/gitjobs-candidate
rustup override set 1.96.1
rustc --version
cargo --version
```

Expected: `rustc 1.96.1`，且Cargo命令成功。

- [ ] **Step 7: Install Node.js 20 and pinned browser-test packages**

```bash
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && . "$NVM_DIR/nvm.sh"
nvm install 20
nvm use 20
node --version
npm --version
```

Expected: Node主版本为`v20`。

- [ ] **Step 8: Install tern 2.3.2 and Tailwind CSS 4.0.17**

```bash
tmp_dir="$(mktemp -d)"
curl -sL https://github.com/jackc/tern/releases/download/v2.3.2/tern_2.3.2_linux_amd64.tar.gz -o "$tmp_dir/tern.tar.gz"
tar -xzf "$tmp_dir/tern.tar.gz" -C "$tmp_dir"
sudo install -m 0755 "$tmp_dir/tern" /usr/local/bin/tern
sudo curl -sL https://github.com/tailwindlabs/tailwindcss/releases/download/v4.0.17/tailwindcss-linux-x64 -o /usr/local/bin/tailwindcss
sudo chmod 0755 /usr/local/bin/tailwindcss
tern --version
tailwindcss --help
```

Expected: tern显示`2.3.2`；Tailwind帮助命令成功。

---

### Task 2: Add repeatable local configuration examples

**Files:**

- Create: `config/examples/server.local.yml`
- Create: `config/examples/tern.tests.local.conf`
- Create: `config/examples/tern.e2e.local.conf`
- Modify: `.gitignore`

**Interfaces:**

- Consumes: GitJobs `Config`结构、tern配置格式。
- Produces: 本地服务器和两个数据库的稳定配置契约。

- [ ] **Step 1: Extend the ignore rules**

使用补丁在`.gitignore`末尾加入：

```gitignore
config/local/
playwright-report/
blob-report/
```

- [ ] **Step 2: Create the server configuration example**

先创建配置目录：

```bash
mkdir -p config/examples
```


然后创建`config/examples/server.local.yml`：

```yaml
db:
  url: postgres://gitjobs:gitjobs-dev-password@127.0.0.1:5432/gitjobs
email:
  from_address: test@example.com
  from_name: GitJobs Local
  smtp:
    host: 127.0.0.1
    port: 2525
    username: test
    password: local-only
  rcpts_whitelist:
    - test@example.com
log:
  format: pretty
server:
  addr: 0.0.0.0:8080
  base_url: http://127.0.0.1:8080
  cookie:
    secure: false
  login:
    email: true
    github: false
    linuxfoundation: false
  oauth2: {}
  oidc: {}
```

- [ ] **Step 3: Create the database migration examples**

创建`config/examples/tern.tests.local.conf`：

```toml
[database]
host = 127.0.0.1
port = 5432
database = tests
user = tests
password = tests
```

创建`config/examples/tern.e2e.local.conf`：

```toml
[database]
host = 127.0.0.1
port = 5432
database = gitjobs
user = gitjobs
password = gitjobs-dev-password
```

- [ ] **Step 4: Verify local files are ignored and examples are valid**

```bash
mkdir -p config/local
cp config/examples/server.local.yml config/local/server.yml
cp config/examples/tern.tests.local.conf config/local/tern.tests.conf
cp config/examples/tern.e2e.local.conf config/local/tern.e2e.conf
git check-ignore config/local/server.yml
git diff --check
```

Expected: `git check-ignore`输出`config/local/server.yml`；`git diff --check`无输出。

- [ ] **Step 5: Commit the local configuration contract**

```bash
git add .gitignore config/examples
git commit -s -m "chore: add repeatable local development config"
```

Expected: 提交只包含`.gitignore`和三个`config/examples`文件。

---

### Task 3: Prove the Rust workspace baseline

**Files:** No tracked files.

**Interfaces:**

- Consumes: Rust 1.96.1、Tailwind CSS 4.0.17、`Cargo.lock`。
- Produces: 可编译、格式正确、无Clippy警告并通过单元测试的Rust基线。

- [ ] **Step 1: Verify formatting without rewriting files**

```bash
cd /mnt/c/Users/25351/gitjobs-candidate
cargo fmt --all -- --check
```

Expected: exit code 0 and no diff.

- [ ] **Step 2: Verify all targets and features with Clippy**

```bash
cargo clippy --all-targets --all-features --locked -- --deny warnings
```

Expected: exit code 0 and no warning promoted to error.

- [ ] **Step 3: Run the complete Rust test suite**

```bash
cargo test --locked
```

Expected: every test group ends with `test result: ok` and zero failed tests.

- [ ] **Step 4: Build both workspace binaries**

```bash
cargo build --locked -p gitjobs-server -p gitjobs-syncer
test -x target/debug/gitjobs-server
test -x target/debug/gitjobs-syncer
```

Expected: both binary checks return exit code 0.

---

### Task 4: Prove migrations and pgTAP database tests

**Files:** No tracked files.

**Interfaces:**

- Consumes: `database/migrations`、`database/tests`、tern 2.3.2。
- Produces: PostgreSQL schema and functions proven by the repository pgTAP suite.

- [ ] **Step 1: Build the repository database test image**

```bash
cd /mnt/c/Users/25351/gitjobs-candidate
docker build -f database/tests/Dockerfile-postgres-db-tests -t gitjobs/postgres-db-tests:local database/tests
```

Expected: image`gitjobs/postgres-db-tests:local`is listed by`docker image ls`.

- [ ] **Step 2: Start an isolated PostgreSQL test container**

```bash
docker rm -f gitjobs-postgres-tests 2>/dev/null || true
docker run --name gitjobs-postgres-tests \
  -e POSTGRES_DB=tests \
  -e POSTGRES_USER=tests \
  -e POSTGRES_PASSWORD=tests \
  -p 5432:5432 \
  -d gitjobs/postgres-db-tests:local
until docker exec gitjobs-postgres-tests pg_isready -U tests -d tests; do sleep 2; done
```

Expected: `pg_isready` reports`accepting connections`.

- [ ] **Step 3: Apply all schema and function migrations**

```bash
cd database/migrations
TERN_CONF=../../config/local/tern.tests.conf ./migrate.sh
cd ../..
```

Expected: output includes`Applying schema migrations`,`Loading functions`, and two`Done`lines.

- [ ] **Step 4: Enable pgTAP and run every database test**

```bash
PGPASSWORD=tests psql -h 127.0.0.1 -p 5432 -U tests -d tests -c 'CREATE EXTENSION IF NOT EXISTS pgtap;'
PGPASSWORD=tests pg_prove \
  --host 127.0.0.1 \
  --port 5432 \
  --dbname tests \
  --username tests \
  --verbose \
  database/tests/schema/*.sql \
  database/tests/functions/*/*.sql
```

Expected: pg_prove ends with`All tests successful`and`Result: PASS`.

- [ ] **Step 5: Release the database-test port after preserving status**

```bash
docker inspect gitjobs-postgres-tests --format '{{.State.Status}}'
docker rm -f gitjobs-postgres-tests
```

Expected: the first command outputs `running`; the named test container is removed so port 5432 is available for Task 5.

---

### Task 5: Prove the complete browser workflow

**Files:** No tracked files.

**Interfaces:**

- Consumes: built server, PostgreSQL container, local server config, E2E seed and Playwright tests.
- Produces: verified authentication, public project board, employer, job-seeker and moderator browser flows.

- [ ] **Step 1: Start an isolated PostGIS E2E database and migrate it**

```bash
docker rm -f gitjobs-postgres-e2e 2>/dev/null || true
docker run --name gitjobs-postgres-e2e \
  -e POSTGRES_DB=gitjobs \
  -e POSTGRES_USER=gitjobs \
  -e POSTGRES_PASSWORD=gitjobs-dev-password \
  -p 5432:5432 \
  -d postgis/postgis:17-3.5
until docker exec gitjobs-postgres-e2e pg_isready -U gitjobs -d gitjobs; do sleep 2; done
cd database/migrations
TERN_CONF=../../config/local/tern.e2e.conf ./migrate.sh
cd ../..
PGPASSWORD=gitjobs-dev-password psql -h 127.0.0.1 -p 5432 -U gitjobs -d gitjobs \
  -f database/tests/data/e2e.sql
```

Expected: `pg_isready` reports `accepting connections`; migrations print two `Done` lines and seed import exits with code 0. The container image matches the repository E2E workflow's PostGIS 17 baseline.

- [ ] **Step 2: Install the pinned, ignored E2E dependencies**

```bash
npm init -y
npm install --save-dev @playwright/test@1.61.1 wait-on@9.0.10
npx playwright install --with-deps chromium firefox webkit
```

Expected: `npm ls @playwright/test wait-on`shows`@playwright/test@1.61.1`and`wait-on@9.0.10`.

- [ ] **Step 3: Start the server and wait for readiness**

```bash
cargo build --locked --bin gitjobs-server
./target/debug/gitjobs-server --config-file config/local/server.yml >/tmp/gitjobs-server.log 2>&1 &
server_pid=$!
echo "$server_pid" >/tmp/gitjobs-server.pid
npx wait-on --timeout 60000 http://127.0.0.1:8080
curl --fail --silent http://127.0.0.1:8080 >/dev/null
```

Expected: wait-on and curl both exit 0.

- [ ] **Step 4: Run all Playwright projects**

```bash
BASE_URL=http://127.0.0.1:8080 npx playwright test --config tests/e2e/playwright.config.ts
```

Expected: Chromium, Firefox and WebKit tests complete with zero failed tests.

- [ ] **Step 5: Stop the local server without deleting test evidence**

```bash
kill "$(cat /tmp/gitjobs-server.pid)"
wait "$(cat /tmp/gitjobs-server.pid)" 2>/dev/null || true
tail -n 100 /tmp/gitjobs-server.log
```

Expected: process stops; the log contains no panic.

- [ ] **Step 6: Preserve the database status and remove the E2E container**

```bash
docker inspect gitjobs-postgres-e2e --format '{{.State.Status}}'
docker logs --tail 100 gitjobs-postgres-e2e
docker rm -f gitjobs-postgres-e2e
```

Expected: the status is `running`, the final database log contains no fatal error, and the named container is removed.

---

### Task 6: Make the go/no-go decision

**Files:** No tracked files.

**Interfaces:**

- Consumes: Tasks 1 through 5 outputs.
- Produces: an explicit decision on whether restaurant-domain adaptation may start.

- [ ] **Step 1: Verify the worktree contains no accidental generated files**

```bash
git status --short
git diff --check
git diff --cached --check
```

Expected: no source-code modifications and no generated files outside ignored paths.

- [ ] **Step 2: Apply the acceptance gate**

Mark Stage 0`PASS`only when all of the following are true:

```text
Rust 1.96.1 formatting check: PASS
Clippy all targets/all features: PASS
Rust unit tests: PASS
gitjobs-server and gitjobs-syncer builds: PASS
All database migrations: PASS
All pgTAP tests: PASS
All Playwright browser projects: PASS
Server smoke check: PASS
No business-source changes: PASS
```

If any line is not`PASS`, Stage 0 is`BLOCKED`. Preserve the exact failing command and output, then invoke`superpowers:systematic-debugging`before proposing a fix.

- [ ] **Step 3: Announce the next plan boundary**

When Stage 0 passes, write and approve `docs/superpowers/plans/2026-07-13-shared-platform-shell.md`, covering Chinese terminology, two-entry navigation, mobile verification login, shared authorization and responsive shell. Do not start project, talent or AI business implementation until that plan has its own approval.

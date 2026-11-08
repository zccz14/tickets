# Tickets

**为 AI Agent 设计的工单系统。** 报障侧的 Agent 在报障人自己的机器上查清现场，修复侧的 Agent 在项目所有者的机器上把问题修掉，两边都通过 HTTP API 说话，人只需要看结论。

- 报障人写一句「哪里不对」就够了：Cybion Thread 会用报障人自己的账号补齐环境、日志、复现步骤，写回「现场证据」。
- 报障人点提交的那一刻，Tickets 用项目所有者的 Cybion 账号**直接推出**一张修复 Thread，把工单、证据和关闭接口一起交给它。没有队列、没有轮询、没有定时任务，也不用选 Worker。
- 单个可执行文件内嵌前端，SQLite 存数据，不需要容器、不需要环境变量。

线上实例：<https://tickets.ntnl.io>（Auth Mini 登录）。

## 一张工单的旅程

```mermaid
sequenceDiagram
    autonumber
    participant R as 报障人
    participant T as Tickets
    participant C as Cybion（报障人的账号）
    participant X as Cybion（项目所有者的账号）

    R->>T: 新建工单：发送第一条消息（创建草稿）
    T->>C: 创建 Thread + 写简报上下文 + 报障人开场
    C->>C: 在报障人的机器上只读排查
    C->>T: 写回「现场证据」（一次性 Reporter 令牌）
    R->>T: 提交工单
    T->>X: 创建修复 Thread + 写派单上下文 + 所有者开场
    X->>T: 读工单与证据，诊断并修复
    X->>T: 写回处理结论并关闭（一次调用，Resolver 令牌）
```

一张工单只有三段文字：**正文**（报障人写的现象）、**现场证据**（报障 Agent 在机器上量到的）、**处理结论**（修复 Agent 做完的）。没有评论区。

两侧的权限由两种 `tk_` 开头的 Agent 令牌限定，两者都只绑定自己那张工单：Reporter 只能写证据，Resolver 只能写结论并关单。重新打开工单或报障 Agent 补写了新证据时，Tickets 会往同一张修复 Thread 里再推一条消息——仍然是推送，不是轮询。

产品语义见 [PRODUCT.md](PRODUCT.md)，实现与部署见 [DESIGN.md](DESIGN.md)。

## 本地开发

需要 Rust 1.93、Node 24+、pnpm 10。

```bash
# 1. 后端：--dev-user 关闭 Auth Mini 校验，并把所有请求视为同一个用户
cargo run -- \
  --data-dir /tmp/tickets-dev \
  --port 8080 \
  --public-base-url http://127.0.0.1:8080 \
  --dev-user 031a393d-8a0a-4ca0-8cb1-313e0d71dfa9

# 2. 前端：Vite 把 /api 代理到 8080
cd web
pnpm install
VITE_DEV_USER=1 pnpm dev
```

打开 <http://127.0.0.1:5173>。`VITE_DEV_USER` 只是让前端跳过 Auth Mini 登录，**具体是谁由后端的 `--dev-user` 决定**；换用户就换参数重启后端。

只在 8080 上预览内嵌版本（构建产物会被 `rust-embed` 打进二进制）：

```bash
cd web && pnpm build && cd ..
cargo run -- --data-dir /tmp/tickets-dev --port 8080 --public-base-url http://127.0.0.1:8080 --dev-user <uid>
```

## 运行参数

| 参数 | 默认值 | 说明 |
| --- | --- | --- |
| `--listen host:port` | `0.0.0.0:8080` | 监听地址 |
| `--port n` | — | 只改端口的快捷方式 |
| `--data-dir path` | `./data` | 数据库与 `secret.key` 所在目录 |
| `--public-base-url url` | 数据库中的值或 `https://tickets.ntnl.io` | 写进简报的对外地址 |
| `--dev-user <subject>` | 无 | 关闭 Auth Mini，仅限本地 |

部署级的地址保存在 SQLite 的 `settings` 表里，启动时读取一次：

| key | 默认值 |
| --- | --- |
| `auth_base_url` | `https://auth.ntnl.io` |
| `linkit_base_url` | `https://linkit.ntnl.io` |
| `cybion_base_url` | `https://cybion.ntnl.io` |
| `public_base_url` | `https://tickets.ntnl.io` |

```bash
sqlite3 /tmp/tickets-dev/tickets.db \
  "INSERT INTO settings(key,value) VALUES('cybion_base_url','https://cybion.example.com') \
   ON CONFLICT(key) DO UPDATE SET value=excluded.value"
```

## HTTP API

浏览器接口由 Auth Mini 保护，`Authorization: Bearer <access token>`；Agent 接口由 Agent 令牌保护，`Authorization: Bearer tk_…`。两条路由空间互不相通。

浏览器侧：

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET | `/api/health`、`/api/meta` | 公开：健康检查与服务地址 |
| GET | `/api/me`、`/api/me/tickets` | 当前用户与其工单 |
| GET/PUT/DELETE | `/api/account/cybion` | 连接 / 断开本人的 Cybion 账号（PUT 自动铸 Key） |
| GET/POST | `/api/projects` | 可见项目列表 / 新建（默认私有） |
| GET/PATCH | `/api/projects/{id}` | 项目详情 / 修改名称、描述、可见性（仅所有者） |
| GET/POST | `/api/projects/{id}/grants` | 私有项目的授权成员 / 添加成员（仅所有者） |
| DELETE | `/api/projects/{id}/grants/{user_id}` | 移除授权成员（仅所有者） |
| GET/POST | `/api/projects/{id}/tickets` | 工单列表（`?status=open\|closed\|draft\|all`）/ 新建（省略 `title` 时取第一条消息的第一行） |
| GET/PATCH | `/api/tickets/{id}` | 工单详情 / 修改标题正文（仅报障人） |
| POST | `/api/tickets/{id}/submit` | 提交草稿：立刻推给修复 Agent |
| POST | `/api/tickets/{id}/close`、`reopen` | 关闭 / 重新打开（重开会再推一次修复 Thread） |
| POST | `/api/tickets/{id}/dispatch` | 派发失败后由所有者重试（仅所有者） |
| GET/POST | `/api/tickets/{id}/thread` | 查看 / 补建现场调查 Thread |
| GET/POST | `/api/tickets/{id}/thread/live` | 实时读取 Thread 历史（`?after=`）/ 报障人直接向 Agent 发消息 |

Agent 侧（两种令牌都只绑定一张工单）：

| 方法 | 路径 | 作用域 |
| --- | --- | --- |
| GET | `/api/agent/whoami` | 两者 |
| GET | `/api/agent/tickets/{id}` | 两者，但只能读自己那张工单 |
| POST | `/api/agent/tickets/{id}/evidence` | 仅 Reporter：写回现场证据 |
| POST | `/api/agent/tickets/{id}/resolve` | 仅 Resolver：写回处理结论并关闭 |

## 测试

```bash
cargo test                      # 单元 + 集成：真路由 + 假 Cybion
cd web && pnpm test             # i18n、样式、面板布局
cd web && pnpm run check        # 测试 + 类型 + 构建
```

## 部署

发布完全由 GitHub Actions 完成，服务器上以 systemd + Caddy 运行，数据目录 `/opt/tickets/data` 不随发布覆盖。

```mermaid
flowchart LR
    A[push main] --> B[tag v0.1.0-N]
    B --> C[Amazon Linux 2023 容器内构建]
    C --> D[GitHub Release + S3]
    D --> E[SSM 下发到 t3.small]
    E --> F[systemd 重启 + 健康检查]
    F --> G[Caddy 自动 HTTPS]
```

- `.github/workflows/pr-check.yml`：PR 的测试、格式、lint 与构建门禁。
- `.github/workflows/release.yml`：合并后打 tag、构建、上传 S3、SSM 部署，失败自动回滚。
- `deploy/bootstrap.sh`：一次性主机初始化（用户、目录、Caddy）。
- `deploy/remote-deploy.sh`：实例上的发布脚本（校验 sha256、同步 unit/Caddyfile、重启、健康检查）。

重新部署一个历史版本：手动触发 `release` 工作流，填 `deploy_tag`。

## 目录结构

```
src/            后端：Axum 路由、SQLite、Auth Mini、Cybion 客户端、两侧简报
web/            前端：React + Vite + Tailwind + shadcn + @zccz14/ux
tests/          集成测试：真路由 + 假 Cybion 服务
deploy/         主机初始化、systemd 单元、Caddyfile、实例发布脚本
.github/        PR 检查与发布工作流
```

## License

MIT

# Tickets 部署（tickets.ntnl.io）

把 `tickets` 部署到 AWS EC2（东京 `ap-northeast-1`），服务器上以 systemd + Caddy 运行，构建与发布全部由 GitHub Actions 完成（GitHub Release / S3 → SSM）。机器清单的机器可读版本见 [aws/tickets.json](aws/tickets.json)。

## 资源

| 项 | 值 |
| --- | --- |
| 实例 | `i-0494f14df1b54a72a`（`tickets-prod`，t3.small，Amazon Linux 2023 x86_64，ap-northeast-1a） |
| 网络 | 默认 VPC `vpc-0d4ea60f1cf653f38`，子网 `subnet-0039e735f257c98a3`，EIP `13.114.60.69` |
| 安全组 | `sg-052205d1cf30bc71f`（`tickets-prod-sg`：80/443 公网；不开 22，运维全走 SSM） |
| 发布桶 | `ntnl-tickets-prod-deploy-524909477426-ap-northeast-1`（私有、SSE-AES256） |
| 发布角色 | `tickets-github-actions-deploy`（GitHub OIDC，仅 `zccz14/tickets` 的 main / `v*` tag） |
| 实例角色 | `tickets-prod-ssm-role`（`tickets-prod-ssm-profile`：SSM Core + 读发布桶） |
| 数据目录 | `/opt/tickets/data`（`tickets.db`、WAL、`secret.key`，发布不覆盖） |
| 入口 | Caddy 反代 `127.0.0.1:8080`，Let's Encrypt 自动签发续期 |

## 首次初始化

实例已经创建好时，把 [`bootstrap.sh`](bootstrap.sh) 经 SSM 送上去执行一次：

```sh
b64=$(base64 < deploy/bootstrap.sh | tr -d '\n')
~/.local/bin/ssm-run i-0494f14df1b54a72a ap-northeast-1 \
  "printf '%s' '$b64' | base64 -d > /tmp/tickets-bootstrap.sh && bash /tmp/tickets-bootstrap.sh tickets.ntnl.io"
```

它会创建 `tickets` 用户与数据目录、安装固定版本的 Caddy、写入 systemd 单元并启用服务。这一步不会部署二进制，第一个版本由 release 工作流推送。

## 发布流程

合并到 `main` 后自动执行 [release.yml](../.github/workflows/release.yml)：

1. **release**：创建 GitHub release `v0.1.0-<run>`；
2. **package**：构建前端，在 `amazonlinux:2023` 容器里编译 x86_64 二进制（避免 CI 的 glibc 比生产新），确认前端确实被内嵌，打包 `tickets-linux-x86_64.tar.gz` + sha256，挂到 release 并上传 S3；
3. **deploy**：GitHub OIDC 换临时角色，`aws ssm send-command` 在实例上执行 [`remote-deploy.sh`](remote-deploy.sh)——从 S3 取包、校验 sha256、按内容差异同步 unit 与 Caddyfile、重启 `tickets`，轮询 `/api/health` 失败则回滚上一个二进制。

重新部署某个历史版本：手动触发 `release` 工作流并填 `deploy_tag`（如 `v0.1.0-3`）。

## 运维

```sh
# 服务与证书状态
aws ssm send-command --region ap-northeast-1 --instance-ids i-0494f14df1b54a72a \
  --document-name AWS-RunShellScript \
  --parameters 'commands=["systemctl status tickets caddy --no-pager; journalctl -u tickets -n 80 --no-pager"]'

# 重启
aws ssm send-command --region ap-northeast-1 --instance-ids i-0494f14df1b54a72a \
  --document-name AWS-RunShellScript --parameters 'commands=["systemctl restart tickets"]'

# 数据目录（不要在实例上做重型扫描，先同步副本）
aws ssm send-command --region ap-northeast-1 --instance-ids i-0494f14df1b54a72a \
  --document-name AWS-RunShellScript --parameters 'commands=["ls -l /opt/tickets/data"]'
```

- 证书：Caddy 自动申请与续期，Cloudflare 记录必须保持 **DNS-only**（灰云）。
- 配置：改仓库里的 `deploy/tickets/*`，合并即生效。
- 数据：`secret.key` 丢了或换了，所有已存的 Cybion Key 都解不开，用户需要重新连接账号。

#!/usr/bin/env bash
# One-time host setup for tickets.ntnl.io on Amazon Linux 2023.
#
# Everything after this script is delivered by `.github/workflows/release.yml`:
# the binary, the systemd unit and the Caddyfile all arrive in the release
# archive, so this only has to prepare the machine, the service user and TLS.
set -euo pipefail

domain="${1:-tickets.ntnl.io}"
caddy_version="2.11.7"
caddy_sha512="a7a433a1b133efc3c8d10eb0b99d52a24b5ef5c322dc77f5282182b1c0402139ab83f3a99f0c52409df77d20123fb0b523edad8a66d8f5e49136197bf61ef0e7"
app_user="tickets"

if [ "$(id -u)" -ne 0 ]; then
  echo "请以 root 运行（SSM 默认就是 root）" >&2
  exit 1
fi

dnf install -y ca-certificates tar gzip shadow-utils

# The service account owns the SQLite database and the AES key beside it.
id -u "$app_user" >/dev/null 2>&1 || useradd \
  --system \
  --home-dir /opt/tickets \
  --shell /usr/sbin/nologin \
  "$app_user"
install -d -o "$app_user" -g "$app_user" -m 0700 /opt/tickets/data

# Caddy is not in the AL2023 repositories, so install the pinned static build
# and verify it against the SHA-512 published with the release.
if ! command -v caddy >/dev/null 2>&1; then
  temporary_dir="$(mktemp -d)"
  trap 'rm -rf "$temporary_dir"' EXIT
  curl --fail --location --retry 5 --retry-all-errors \
    --output "$temporary_dir/caddy.tar.gz" \
    "https://github.com/caddyserver/caddy/releases/download/v${caddy_version}/caddy_${caddy_version}_linux_amd64.tar.gz"
  printf '%s  %s\n' "$caddy_sha512" "$temporary_dir/caddy.tar.gz" | sha512sum --check -
  tar -xzf "$temporary_dir/caddy.tar.gz" -C "$temporary_dir" caddy
  install -m 0755 "$temporary_dir/caddy" /usr/local/bin/caddy
fi

id -u caddy >/dev/null 2>&1 || useradd \
  --system \
  --home-dir /var/lib/caddy \
  --shell /usr/sbin/nologin \
  caddy
install -d -o caddy -g caddy -m 0700 /var/lib/caddy
install -d -m 0755 /etc/caddy

install -m 0644 /dev/stdin /etc/systemd/system/caddy.service <<'UNIT'
[Unit]
Description=Caddy web server
After=network-online.target
Wants=network-online.target

[Service]
User=caddy
Group=caddy
ExecStart=/usr/local/bin/caddy run --environ --config /etc/caddy/Caddyfile
ExecReload=/usr/local/bin/caddy reload --config /etc/caddy/Caddyfile --force
TimeoutStopSec=5s
LimitNOFILE=1048576
PrivateTmp=true
ProtectSystem=full
AmbientCapabilities=CAP_NET_BIND_SERVICE
Restart=on-failure

[Install]
WantedBy=multi-user.target
UNIT

if [ ! -f /etc/caddy/Caddyfile ]; then
  install -m 0644 /dev/stdin /etc/caddy/Caddyfile <<CADDY
${domain} {
	encode zstd gzip
	reverse_proxy 127.0.0.1:8080
}
CADDY
  chown caddy:caddy /etc/caddy/Caddyfile
fi

# The unit is replaced on every deploy; installing it here lets `enable` run
# before the first release exists.
install -m 0644 /dev/stdin /etc/systemd/system/tickets.service <<'UNIT'
[Unit]
Description=Tickets (tickets.ntnl.io)
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=tickets
Group=tickets
WorkingDirectory=/opt/tickets
ExecStart=/usr/local/bin/tickets --listen 127.0.0.1:8080 --data-dir /opt/tickets/data
Restart=on-failure
RestartSec=5s
UMask=0077
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictSUIDSGID=true
LockPersonality=true
ReadWritePaths=/opt/tickets/data

[Install]
WantedBy=multi-user.target
UNIT

systemctl enable amazon-ssm-agent.service
systemctl restart amazon-ssm-agent.service
systemctl daemon-reload
systemctl enable tickets.service
systemctl enable caddy.service
systemctl restart caddy.service
caddy validate --config /etc/caddy/Caddyfile

echo "主机已就绪：${domain}，接下来由 release 工作流推送第一个版本。"

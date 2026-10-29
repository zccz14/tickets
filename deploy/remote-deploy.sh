#!/usr/bin/env bash
# Runs on the production instance through SSM. Args: <tag>; env: BUCKET.
#
# The data directory is never touched: it holds tickets.db, its WAL and the
# AES key that decrypts every stored Cybion API key.
set -euo pipefail

tag="${1:?release tag is required}"
bucket="${BUCKET:?deploy bucket is required}"
archive="tickets-linux-x86_64.tar.gz"
health_url="http://127.0.0.1:8080/api/health"
working_dir="$(mktemp -d)"
backup="/usr/local/bin/tickets.backup"

cleanup() {
  rm -rf "$working_dir"
}
trap cleanup EXIT

cd "$working_dir"
aws s3 cp "s3://${bucket}/releases/${tag}/${archive}" "$archive" --only-show-errors
aws s3 cp "s3://${bucket}/releases/${tag}/${archive}.sha256" "$archive.sha256" --only-show-errors
sha256sum --check "$archive.sha256"
tar -xzf "$archive"
test -x release/bin/tickets

install -d -o tickets -g tickets -m 0700 /opt/tickets/data

if ! cmp -s release/tickets.service /etc/systemd/system/tickets.service; then
  install -m 0644 release/tickets.service /etc/systemd/system/tickets.service
  systemctl daemon-reload
fi
if ! cmp -s release/Caddyfile /etc/caddy/Caddyfile; then
  install -m 0644 -o caddy -g caddy release/Caddyfile /etc/caddy/Caddyfile
  caddy validate --config /etc/caddy/Caddyfile
  systemctl reload caddy || systemctl restart caddy
fi

if [ -x /usr/local/bin/tickets ]; then
  cp /usr/local/bin/tickets "$backup"
fi
install -m 0755 release/bin/tickets /usr/local/bin/tickets
systemctl restart tickets.service

healthy=0
for _ in $(seq 1 60); do
  if curl --fail --silent --max-time 2 "$health_url" >/dev/null; then
    healthy=1
    break
  fi
  sleep 2
done

if [ "$healthy" -ne 1 ]; then
  echo "健康检查失败（${tag}），回滚上一个二进制" >&2
  journalctl -u tickets.service --no-pager 2>&1 | tail -40 >&2 || true
  if [ -x "$backup" ]; then
    install -m 0755 "$backup" /usr/local/bin/tickets
    systemctl restart tickets.service
  fi
  exit 1
fi

echo "tickets ${tag} 已部署并通过健康检查"
systemctl status tickets.service --no-pager | head -12

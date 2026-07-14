# 子机器（Worker Host）接入说明

本文档面向**子机器**：本机只跑 Desktop，连到已有主机器上的 Remote + Relay，作为可被面板调度的 Host。

主机器全栈启动见 [`vk-usage.md`](vk-usage.md)。配对 UI 见 [`remote-access.mdx`](remote-access.mdx)。

## 架构

| 角色 | 跑什么 | 不要做什么 |
|------|--------|------------|
| **主机器** | `vk-start`：Remote + Relay（+ 可选本机 Desktop） | — |
| **子机器** | 仅 Desktop（`dev-full-local` / `pnpm run dev`） | **不要** `vk-start`（会再起一套 Docker Remote） |

```
主机器 Remote + Relay  ← 网络（建议 Tailscale HTTP）→  子机器 Desktop（Host）
面板与数据在主机器；任务在各 Host 本机执行
```

默认端口（见 `scripts/vk-ports.sh`）：

| 服务 | 端口 |
|------|------|
| Desktop Web | 13001 |
| Desktop API | 13002 |
| Remote | 13000 |
| Relay | 18082 |

## 前置：从主机器拿到的信息

- 主机器可达地址：Tailscale IP 或主机名（下文记为 `<主机地址>`）
- Remote：`http://<主机地址>:13000`
- Relay：`http://<主机地址>:18082`
- 与主机器 Remote **同一登录账号**（自托管默认见主机器 `crates/remote/.env.remote`）

> **重要**：子机服务端连 Remote/Relay 必须用 **HTTP**（Tailscale IP/主机名 + 端口），不要指向主机器的 HTTPS Caddy 前门，否则易出现 token / relay 失败。

## 1. 网络打通

子机与主机器需在同一可达网络（推荐 Tailscale）。

```bash
curl -s -o /dev/null -w "%{http_code}\n" --max-time 3 "http://<主机地址>:13000/v1/health"
curl -s -o /dev/null -w "%{http_code}\n" --max-time 3 "http://<主机地址>:18082/health"
# 期望均为 200
```

## 2. 工具链与依赖

- Node 22+、pnpm
- Rust stable、`cargo-watch`
- 克隆同一仓库后：`pnpm i`

### Linux / WSL 编译依赖

有 sudo 时：

```bash
sudo apt-get install -y libssl-dev pkg-config
```

无 sudo、本机有 conda 时，可用 conda 的 OpenSSL（避免污染全局）：

```bash
# 按本机 conda 前缀调整路径
export OPENSSL_DIR="${CONDA_PREFIX:-$HOME/anaconda3}"
export OPENSSL_LIB_DIR="$OPENSSL_DIR/lib"
export OPENSSL_INCLUDE_DIR="$OPENSSL_DIR/include"
export PKG_CONFIG_PATH="$OPENSSL_DIR/lib/pkgconfig"
export LD_LIBRARY_PATH="$OPENSSL_DIR/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export LIBCLANG_PATH="$OPENSSL_DIR/lib"
# 按本机 gcc 版本调整 include 路径
export BINDGEN_EXTRA_CLANG_ARGS="-isystem/usr/lib/gcc/x86_64-linux-gnu/13/include -isystem/usr/include"
# 不要设置 CPATH / C_INCLUDE_PATH（会干扰 bindgen）
```

## 3. 初始化本地配置

```bash
# 若 scripts/setup-dev-environment.js 端口探测卡住，可手动：
cp -a dev_assets_seed/. dev_assets/
```

确保 `dev_assets/config.json`（及若使用的 `~/.vk-kanban/dev_assets/config.json`）中：

- `relay_enabled`: `true`
- `remote_onboarding_acknowledged`: `true`
- `host_nickname`: 便于识别的名称（如 `worker-linux-1`）

`scripts/dev-full-local.sh` 启动时也会把 `relay_enabled` / onboarding 打开。

## 4. 启动 Desktop（指向主机器）

```bash
cd /path/to/hyper-vibekanban

export VK_SHARED_API_BASE="http://<主机地址>:13000"
export VK_SHARED_RELAY_API_BASE="http://<主机地址>:18082"
export VITE_VK_SHARED_API_BASE="http://<主机地址>:13000"
export VITE_RELAY_API_BASE_URL="http://<主机地址>:18082"

# 代理环境下必须绕过主机器与本机回环，否则易 502
export NO_PROXY="localhost,127.0.0.1,::1,<主机地址>"
export no_proxy="$NO_PROXY"

# 建议固定端口，避免部分环境下端口探测挂起
export FRONTEND_PORT=13001
export BACKEND_PORT=13002

# 不要 vk-start
bash scripts/dev-full-local.sh
```

浏览器打开：`http://localhost:13001`

可将上述 `export` 写入本机 shell 配置或启动脚本，长期复用。

## 5. 配对成 Host

1. 子机 Desktop 登录与主机器 **同一账号**
2. Settings → **Remote Access** → 选 **Host** → 确认 relay / nickname → **Show pairing code**
3. 在**主机器** Remote 面板（`http://<主机地址>:13000` 或主机器已配置的 HTTPS 入口）→ Remote Access → **Link a host** → 输入配对码
4. 主机器 AppBar / Host 列表应显示该子机为 online

## 6. 验收

- [ ] 主机器能看到子机 Host 且状态正常
- [ ] 子机日志有 Relay 注册成功相关输出
- [ ] 子机 `localhost:13001` 可登录并看到与主机器一致的项目数据

> **已知限制**：部分客户端从看板直接创建 workspace 时可能仍默认本机 Host。跨机执行请确认走带 `hostId` 的路径，或使用 Remote Web / Host 工作区入口。详见多机 Host 选择相关 issue。

## 日常运维

| 场景 | 操作 |
|------|------|
| 日常启动 | 带上第 4 节环境变量后 `bash scripts/dev-full-local.sh` |
| 停止 | 停掉本机 `pnpm run dev` / 相关 cargo、vite 进程即可；**不要**对主机器执行 `vk-stop --all` |
| 主机器地址变更 | 更新四个 `VK_*` / `VITE_*` 与 `NO_PROXY` 后重启 Desktop |
| 主机器重启 Remote | 子机一般无需重建；若断连，重启 Desktop 即可 |

## 踩坑速查

| 现象 | 处理 |
|------|------|
| 子机又起了一套 Remote | 不要用 `vk-start`，用 `dev-full-local` |
| `/api/auth/token` 或 relay 502 | `VK_SHARED_*` 用 HTTP 主地址；检查 `NO_PROXY` |
| 编译缺 OpenSSL | 装 `libssl-dev`，或按第 2 节用 conda OpenSSL |
| bindgen 找不到系统头 | 设置 `BINDGEN_EXTRA_CLANG_ARGS`；勿污染 `CPATH` |
| `setup-dev-environment` 卡住 | 固定 `FRONTEND_PORT` / `BACKEND_PORT`；手动拷 `dev_assets_seed` |
| 健康检查非 200 | 先修 Tailscale / 防火墙，再启 Desktop |

## 与主机器文档的关系

- 主机器一键启停、端口、手机访问：[`vk-usage.md`](vk-usage.md)
- Host 配对 UI：[`remote-access.mdx`](remote-access.mdx)
- 手机 Tailscale 慢：[`vk-mobile-tailscale-troubleshooting.md`](vk-mobile-tailscale-troubleshooting.md)

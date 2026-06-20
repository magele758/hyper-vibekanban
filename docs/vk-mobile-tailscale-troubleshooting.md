# 手机访问慢排查:Tailscale 直连 vs DERP 中继

> 配套文档:[`vk-usage.md`](vk-usage.md)(端口/启动)、根目录 `CLAUDE.md`「Local Dev Stack」一节(手机 HTTPS 前门 `VK_MOBILE`)。
>
> 适用场景:手机经 Tailscale 访问 Vibe Kanban(Desktop `:13001` / Remote `:13000` / 手机 HTTPS 前门 `:13444`)时**很慢或剧烈卡顿**,但**同一手机连 WiFi 就很快**。

## TL;DR

- **慢的根因不是 app,是 Tailscale 没打通点对点直连,流量退回了 DERP 中继(默认绕到东京),延迟 300ms+ 且剧烈抖动。**
- `VK_MOBILE=1` / HTTP/2 前门只解决「浏览器并发连接数」问题(一个看板要 ~9 条 Electric 长连接),**解决不了传输层延迟**。底层每个字节还是走中继。
- **解决办法:在家庭路由器开启 UPnP**,让 Mac 侧拿到稳定公网端口映射,5G 手机就能直拨建立直连。

## 一、为什么会慢(原理)

手机在 5G 蜂窝网络下处于**运营商级 NAT(CGNAT)**后面,只能主动往外拨、无法被拨入。如果 Mac 侧也没有可用的端口映射(路由器没开 UPnP/NAT-PMP),两边都开不了「洞」,Tailscale 只能退回 **DERP 中继**:

```
慢(中继):  iPhone(5G) ──> 东京 DERP ──> Mac(家里)     // 出境绕一圈,300ms+,抖动大
快(直连):  iPhone ───────────────────> Mac            // 点对点,几十 ms
```

关键认知:**让「Mac 能被拨入」比「手机能被拨入」容易**。手机在 CGNAT 后基本无解,但 Mac 在家庭路由器后,只要开 UPnP 拿到固定公网 `ip:端口`,手机就能直接拨过来 —— 这就是开 UPnP 能修好 5G 的原因。

## 二、诊断步骤

> ⚠️ **必须用 Tailscale 的完整路径二进制**:`/Applications/Tailscale.app/Contents/MacOS/Tailscale`。
> App Store 版的 GUI 二进制经 PATH 软链接调用会 SIGTRAP 崩溃(这点 `CLAUDE.md` 也强调过)。

```bash
TS="/Applications/Tailscale.app/Contents/MacOS/Tailscale"

# 1) 看手机当前是 direct 还是 relay
"$TS" status | grep -i iphone

# 2) 看本机 NAT / 端口映射能力
"$TS" netcheck | grep -E "UDP:|PortMapping|MappingVaries|Nearest DERP"

# 3) 直接 ping 手机,看走 direct 还是 DERP(把 IP 换成你手机的 100.x)
"$TS" ping --c 10 100.x.y.z
```

### 怎么读输出

| 字段 | ❌ 故障(走中继) | ✅ 正常(直连) |
|------|----------------|---------------|
| `status` 里手机那行 | `active; relay "tok", ...` | `active; direct [2409:...]:41641, ...` |
| `netcheck` 的 `PortMapping:` | 空(什么都没探测到) | `UPnP, NAT-PMP, PCP` |
| `ping` 结果 | `via DERP(tok) in 301ms ~ 2.5s`,末尾 `direct connection not established` | `pong ... direct ...`(低延迟) |

## 三、本次真实排查记录(2026-06-20)

**故障态(开 UPnP 之前):**

```
# status
100.x.y.z  my-iphone  ...  active; relay "tok", tx 244556 rx 204436

# netcheck
* PortMapping:            (空)
* MappingVariesByDestIP: false       # Mac 侧 NAT 友好,不是对称 NAT
* Nearest DERP: San Francisco        # 国际出口本身就慢:sfo 160ms / hkg 285ms

# ping —— 10 次全部走中继,延迟 301ms~2.5s 剧烈抖动
pong from my-iphone via DERP(tok) in 1.393s
pong from my-iphone via DERP(tok) in 389ms
... (略) ...
direct connection not established
```

**修复动作:在家庭路由器(网关 `192.168.x.x`)开启 UPnP。**

**正常态(开 UPnP 之后):**

```
# status —— 变成 direct(本次经 IPv6 直连)
100.x.y.z  my-iphone  ...  active; direct [2409:xxxx:xxxx:xxxx:...]:41641, tx 6041192 rx 1284828

# netcheck —— 三种端口映射协议都探测到了
* UDP: true
* PortMapping: UPnP, NAT-PMP, PCP
```

结论:开 UPnP 后 `PortMapping` 立即生效,手机 5G 访问从「绕东京中继」变为「直连」,速度恢复正常。

## 四、如果开了 UPnP 还是 `PortMapping:` 空

按顺序排查:

1. **重启路由器** —— 很多路由器开 UPnP 开关后要重启才真正起服务。
2. **确认 Mac 直连主路由,不是 Mesh 子节点** —— Mesh/二级 AP 可能没把 UPnP 透传到主网关。
3. **排查双重 NAT** —— 若是「光猫拨号 + 路由器」两层 NAT,只在内层路由器开 UPnP 无效;需光猫改桥接,或在光猫层也处理。
4. 改完后重跑 `netcheck`,确认 `PortMapping` 出现 `UPnP`。
5. **真正验证必须手机切回 5G** 再 `tailscale ping`(手机在 WiFi 时一定显示 direct,测不出 5G 的问题)。

## 五、安全说明(开端口会不会被攻击?)

**不会暴露你的应用或局域网。** 给 Tailscale 开的端口上跑的是 **WireGuard 流量:全程加密 + 公钥认证**。外部扫到这个端口,发任何没有你 tailnet 私钥签名的包都会被**静默丢弃**,没有可攻击的握手面。kanban 应用(`13000/13001/...`)**始终只在 tailnet 内可达,不上公网**。

按「家庭网络暴露」从低到高:

| 方案 | 家里入站 | 暴露内容 | 风险 |
|------|---------|---------|------|
| 自建 DERP 中继(见下) | 无(Mac 主动外连) | 零 | 几乎为零 ✅ |
| 手动只转发 1 个固定 UDP 端口给 Tailscale | 1 个 UDP 端口 | 仅加密 WireGuard | 很低 ✅ |
| 路由器全局 UPnP | UPnP 服务 | 同上;但**任何局域网设备**都能自助开端口 | 低~中 ⚠️ |
| DMZ 主机 | **全部端口** | 那台机器完全裸奔 | 高 ❌ 别用 |
| Cloudflare Tunnel / frp 把 app 公网化 | 视配置 | **应用本身上公网** | 中~高,需自扛鉴权 ❌ |

- **全局 UPnP 的唯一隐患**不在 Tailscale,而在于:开了之后家里**任何设备(IoT/摄像头等)都能自动给自己开公网端口**。若家里有不可信设备,更稳的做法是把 Tailscale 固定到某端口(如 `41641`)后,在路由器**只手动转发那一个 UDP 端口**,然后关掉全局 UPnP。
- **永远不要用 DMZ** 来「打通」NAT。

## 六、兜底方案:自建国内 DERP 中继

如果 UPnP 实在搞不定(国内宽带 + 5G CGNAT 不一定每次都能直连),最稳的兜底是**自建 DERP**:在一台国内/低延迟 VPS 上跑 `derper`,改 Tailscale ACL 指过去。

- 家里**一个入站端口都不用开**(Mac 主动外连 VPS),安全性最高。
- 即便打不通直连,中继路径也变成「手机 → 国内 VPS → Mac」,而不是绕东京,延迟从 300ms+ 压到几十 ms。
- 代价:需要一台 VPS 和少量配置。

## 附:相关地址速查

- 手机 HTTPS 前门(需 `VK_MOBILE=1`):`https://<tailscale-hostname>:13444`,Relay `:18443`
- Tailscale 主机名/IP 由 `vk-start` 启动横幅自动打印;手机配 Relay 用**当前页面的 Tailscale 主机名/IP**,不要用局域网 IP。
- 本机直连诊断二进制:`/Applications/Tailscale.app/Contents/MacOS/Tailscale`(必须用完整路径)。

# Kubernetes

数据服务清单，用已发布的镜像，不在这里构建。

```bash
# 先改 secret.yaml、configmap.yaml
kubectl apply -k deploy/k8s
kubectl -n vibekanban get pods,svc
```

集群需要默认 StorageClass。对外访问时只给 Remote 和 Relay 加 Ingress，不要把 Postgres 或 Electric 暴露到集群外。

## 副本

| 组件 | 副本 | Service |
| --- | --- | --- |
| Postgres | 1 | ClusterIP。不要改成 NodePort 或 LoadBalancer。业务数据只在 PVC `postgres-data`。 |
| Electric | 1 | ClusterIP，只给集群内 Remote 用。它独占 Postgres 逻辑复制槽，不要加副本。更新用 Recreate。 |
| Remote API | 2 | ClusterIP，在副本前负载均衡。每个副本同一份 `VIBEKANBAN_REMOTE_JWT_SECRET`，连同一个 Postgres。 |
| Relay | 1 | ClusterIP，`sessionAffinity: ClientIP`。 |

Relay 先保持 1 个副本。活跃 Host 注册表还在进程内存里，进 Postgres 之后才能加副本。长连接靠 ClientIP 会话保持，重连会回到同一后端。更新策略是 Recreate，避免滚动时出现第二个进程。

Electric 的 PVC 只是这个单副本的 shape 缓存，要和复制槽留在一起。

## 镜像

- `ghcr.io/magele758/hyper-vibekanban-remote:latest`
- `ghcr.io/magele758/hyper-vibekanban-relay:latest`
- `electricsql/electric:1.4.13`
- `postgres:16-alpine`

## 启动前

`secret.yaml`：

- `VIBEKANBAN_REMOTE_JWT_SECRET` 用 `openssl rand -base64 48`。标准 base64，解码后至少 32 字节。Remote 和 Relay 读同一条。
- `POSTGRES_PASSWORD` 和 `ELECTRIC_ROLE_PASSWORD` 只用字母和数字。第一次启动前改好。数据目录初始化之后，只改 Secret 不会改掉库里已有口令。
- 至少一种登录。默认是 `SELF_HOST_LOCAL_AUTH_EMAIL` / `SELF_HOST_LOCAL_AUTH_PASSWORD`。改用 GitHub 或 Google 时，把 `GITHUB_OAUTH_CLIENT_ID`、`GITHUB_OAUTH_CLIENT_SECRET` 或 `GOOGLE_OAUTH_CLIENT_ID`、`GOOGLE_OAUTH_CLIENT_SECRET` 写进同一个 Secret。

`POSTGRES_USER` 和 `POSTGRES_DB` 保持 `remote`。`configmap.yaml` 的 `SERVER_PUBLIC_BASE_URL` 改成浏览器访问 Remote 的地址。

## 健康检查

探针打容器端口，不经过集群外入口：

- Remote `8081` `/v1/health`
- Electric `3000` `/v1/health`
- Relay `8082` `/health`
- Postgres `pg_isready`

Electric 和 Relay 会等到 Remote 的 `/v1/health` 成功后再启动。Remote 先跑迁移并创建 `electric_sync`。

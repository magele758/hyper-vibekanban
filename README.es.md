<p align="center">
  <a href="https://vibekanban.com">
    <picture>
      <source srcset="packages/public/vibe-kanban-logo-dark.svg" media="(prefers-color-scheme: dark)">
      <source srcset="packages/public/vibe-kanban-logo.svg" media="(prefers-color-scheme: light)">
      <img src="packages/public/vibe-kanban-logo.svg" alt="Vibe Kanban Logo">
    </picture>
  </a>
</p>

<p align="center">Saca 10 veces más partido a Claude Code, Gemini CLI, Codex, Amp y otros agentes de código...</p>
<p align="center">
  <a href="https://www.npmjs.com/package/vibe-kanban"><img alt="npm" src="https://img.shields.io/npm/v/vibe-kanban?style=flat-square" /></a>
  <a href="https://github.com/BloopAI/vibe-kanban/blob/main/.github/workflows/publish.yml"><img alt="Estado del build" src="https://img.shields.io/github/actions/workflow/status/BloopAI/vibe-kanban/.github%2Fworkflows%2Fpublish.yml" /></a>
  <a href="https://deepwiki.com/BloopAI/vibe-kanban"><img src="https://deepwiki.com/badge.svg" alt="Ask DeepWiki"></a>
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-Hans.md">简体中文</a> |
  <a href="README.zh-Hant.md">繁體中文</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.ko.md">한국어</a> |
  <a href="README.fr.md">Français</a>
</p>

> **Aviso:** Vibe Kanban ha anunciado su cierre. [Lee el comunicado](https://www.vibekanban.com/blog/shutdown). El proyecto sigue siendo de código abierto y completamente funcional para autoalojamiento local.

![](packages/public/vibe-kanban-screenshot-overview.png)

## Descripción general

Vibe Kanban es una herramienta de gestión de proyectos local-first diseñada para desarrolladores que trabajan con agentes de codificación con IA. Optimiza el ciclo **planificar → ejecutar → revisar** para que puedas entregar código más rápido.

- **Planifica con issues de kanban** — crea, prioriza y gestiona tarjetas de tareas en un tablero kanban
- **Ejecuta agentes de IA en Workspaces** — cada Workspace crea automáticamente un git worktree aislado, lanza el agente elegido y transmite sus logs en tiempo real
- **Revisa diffs y añade comentarios en línea** — examina cada línea modificada, anótala y envía el feedback al agente sin salir de la UI
- **Vista previa de la aplicación** — navegador integrado con DevTools, inspección de elementos y emulación de dispositivos
- **Más de 10 agentes de IA compatibles** — Claude Code, OpenAI Codex, Gemini CLI, GitHub Copilot, Amp, Cursor Agent CLI, OpenCode, Factory Droid, Claude Code Router (CCR), Qwen Code
- **Crea PRs y fusiona** — abre PRs con descripciones generadas por IA, revísalas en GitHub/Azure y fusiona

![](packages/public/vibe-kanban-screenshot-workspace.png)

## Inicio rápido

Primero, autentícate con tu agente de IA preferido. Luego ejecuta:

```bash
npx vibe-kanban
```

Solo ese comando. Vibe Kanban inicia un servidor local y abre el navegador automáticamente.

## Cómo funciona

### Conceptos clave

| Concepto | Descripción |
|----------|-------------|
| **Project (Proyecto)** | Un repositorio git en tu máquina local |
| **Issue (Tarea)** | Una tarjeta de tarea en el tablero kanban (título + descripción + prioridad + etiquetas) |
| **Workspace (Espacio de trabajo)** | Entorno de ejecución aislado — git worktree + agente de IA + servidor de desarrollo opcional |

### Flujo de trabajo típico

1. **Crea un proyecto** — conecta Vibe Kanban a un repositorio git local
2. **Añade issues** — describe el trabajo pendiente en el tablero kanban
3. **Inicia un Workspace** — elige el agente, la rama y los scripts de configuración/limpieza; el git worktree se crea automáticamente
4. **Observa al agente trabajar** — streaming de logs en tiempo real en la vista del Workspace
5. **Revisa el diff** — vista unificada o lado a lado con comentarios a nivel de línea
6. **Itera** — envía tus comentarios de revisión; el agente los lee y continúa
7. **Entrega** — crea una PR con descripción generada por IA, revísala en GitHub y fusiona

## Agentes de codificación compatibles

| Agente | Proveedor |
|--------|-----------|
| Claude Code | Anthropic |
| OpenAI Codex CLI | OpenAI |
| Gemini CLI | Google |
| GitHub Copilot CLI | GitHub |
| Amp | Sourcegraph |
| Cursor Agent CLI | Anysphere |
| OpenCode | SST |
| Factory Droid | Factory AI |
| Claude Code Router (CCR) | Comunidad |
| Qwen Code | Alibaba |

Consulta la [documentación oficial](https://vibekanban.com/docs/supported-coding-agents) para instrucciones de instalación y autenticación de cada agente.

## Servidor MCP

Vibe Kanban incluye un servidor [MCP (Model Context Protocol)](https://modelcontextprotocol.io/) local que permite a clientes externos (Claude Desktop, Raycast, etc.) gestionar issues y Workspaces de forma programática.

```bash
# Iniciar el servidor MCP
npx vibe-kanban --mcp
```

O añádelo a la configuración MCP de tu agente:

```json
{
  "mcpServers": {
    "vibe_kanban": {
      "command": "npx",
      "args": ["-y", "vibe-kanban@latest", "--mcp"]
    }
  }
}
```

## Referencia de CLI

```bash
npx vibe-kanban               # Inicia la UI local (por defecto)
npx vibe-kanban --mcp         # Inicia el servidor MCP stdio
npx vibe-kanban review        # Ejecuta la CLI de revisión de código
npx vibe-kanban --help
npx vibe-kanban --version
```

## Documentación

Visita el [sitio web oficial](https://vibekanban.com/docs) para documentación completa y guías de usuario.

## Autoalojamiento

¿Quieres alojar tu propia instancia de Vibe Kanban Cloud? Consulta la [guía de autoalojamiento](https://vibekanban.com/docs/self-hosting/deploy-docker).

## Soporte

Usa [GitHub Discussions](https://github.com/BloopAI/vibe-kanban/discussions) para solicitudes de funcionalidades y [GitHub Issues](https://github.com/BloopAI/vibe-kanban/issues) para reportar bugs.

## Contribuciones

Por favor, abre un [GitHub Discussion](https://github.com/BloopAI/vibe-kanban/discussions) o únete a [Discord](https://discord.gg/AC4nwVtJM3) antes de enviar una PR para alinear los detalles de implementación con el roadmap existente.

---

## Desarrollo

### Requisitos previos

- [Rust](https://rustup.rs/) (última versión estable)
- [Node.js](https://nodejs.org/) (≥ 20)
- [pnpm](https://pnpm.io/) (≥ 8)

```bash
cargo install cargo-watch
cargo install sqlx-cli
pnpm i
```

### Iniciar el servidor de desarrollo

```bash
pnpm run dev
```

Inicia el backend Rust (recarga en caliente con `cargo-watch`) y el servidor de desarrollo Vite del frontend de forma concurrente. En el primer arranque, se copia una base de datos SQLite vacía desde `dev_assets_seed/`.

### Compilar solo el frontend

```bash
cd packages/local-web
pnpm run build
```

### Compilar desde el código fuente (genera el paquete npx-cli)

```bash
./local-build.sh
# Prueba el resultado:
cd npx-cli && node bin/cli.js
```

### Comprobación de tipos y linting

```bash
pnpm run check   # TypeScript (todos los paquetes) + Rust cargo check
pnpm run lint    # ESLint + cargo clippy
pnpm run format  # Prettier + cargo fmt
```

### Regenerar tipos TypeScript compartidos

```bash
pnpm run generate-types
```

Los tipos se derivan de estructuras Rust mediante [ts-rs](https://github.com/Aleph-Alpha/ts-rs). **No edites** `shared/types.ts` directamente — edita `crates/server/src/bin/generate_types.rs`.

### Variables de entorno

| Variable | Momento | Por defecto | Descripción |
|----------|---------|-------------|-------------|
| `PORT` | Ejecución | Auto | Puerto del servidor en producción. En desarrollo: puerto del frontend (backend = PORT+1) |
| `FRONTEND_PORT` | Ejecución | `3000` | Puerto de Vite en modo desarrollo |
| `BACKEND_PORT` | Ejecución | `0` (auto) | Puerto del backend en modo desarrollo |
| `HOST` | Ejecución | `127.0.0.1` | Dirección de enlace del backend |
| `VK_ALLOWED_ORIGINS` | Ejecución | — | Orígenes permitidos (separados por comas), obligatorio detrás de un proxy inverso |
| `DISABLE_WORKTREE_CLEANUP` | Ejecución | — | Desactiva la limpieza automática de git worktrees (para depuración) |
| `POSTHOG_API_KEY` | Compilación | — | Clave de analíticas PostHog (desactiva analíticas si está vacío) |

#### Detrás de un proxy inverso

Configura `VK_ALLOWED_ORIGINS` con la URL de origen completa de tu frontend para evitar errores `403 Forbidden`:

```bash
VK_ALLOWED_ORIGINS=https://vk.example.com npx vibe-kanban
```

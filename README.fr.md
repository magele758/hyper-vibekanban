<p align="center">
  <a href="https://vibekanban.com">
    <picture>
      <source srcset="packages/public/vibe-kanban-logo-dark.svg" media="(prefers-color-scheme: dark)">
      <source srcset="packages/public/vibe-kanban-logo.svg" media="(prefers-color-scheme: light)">
      <img src="packages/public/vibe-kanban-logo.svg" alt="Vibe Kanban Logo">
    </picture>
  </a>
</p>

<p align="center">Tirez 10 fois plus parti de Claude Code, Gemini CLI, Codex, Amp et autres agents de code...</p>
<p align="center">
  <a href="https://www.npmjs.com/package/vibe-kanban"><img alt="npm" src="https://img.shields.io/npm/v/vibe-kanban?style=flat-square" /></a>
  <a href="https://github.com/BloopAI/vibe-kanban/blob/main/.github/workflows/publish.yml"><img alt="État du build" src="https://img.shields.io/github/actions/workflow/status/BloopAI/vibe-kanban/.github%2Fworkflows%2Fpublish.yml" /></a>
  <a href="https://deepwiki.com/BloopAI/vibe-kanban"><img src="https://deepwiki.com/badge.svg" alt="Ask DeepWiki"></a>
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-Hans.md">简体中文</a> |
  <a href="README.zh-Hant.md">繁體中文</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.ko.md">한국어</a> |
  <a href="README.es.md">Español</a>
</p>

> **Remarque :** Vibe Kanban a annoncé sa fermeture. [Lire l'annonce](https://www.vibekanban.com/blog/shutdown). Le projet reste open source et entièrement fonctionnel pour l'auto-hébergement local.

![](packages/public/vibe-kanban-screenshot-overview.png)

## Présentation

Vibe Kanban est un outil de gestion de projets local-first conçu pour les développeurs qui travaillent avec des agents de codage IA. Il optimise le cycle **planifier → exécuter → réviser** pour livrer du code plus rapidement.

- **Planifiez avec des issues kanban** — créez, hiérarchisez et gérez des cartes de tâches sur un tableau kanban
- **Exécutez des agents IA dans des Workspaces** — chaque Workspace crée automatiquement un git worktree isolé, lance l'agent choisi et diffuse ses logs en temps réel
- **Révisez les diffs et ajoutez des commentaires inline** — examinez chaque ligne modifiée, annotez-la et renvoyez le feedback à l'agent sans quitter l'UI
- **Aperçu de l'application** — navigateur intégré avec DevTools, inspection d'éléments et émulation d'appareils
- **Plus de 10 agents IA supportés** — Claude Code, OpenAI Codex, Gemini CLI, GitHub Copilot, Amp, Cursor Agent CLI, OpenCode, Factory Droid, Claude Code Router (CCR), Qwen Code
- **Créez des PRs et fusionnez** — ouvrez des PRs avec des descriptions générées par IA, révisez sur GitHub/Azure et fusionnez

![](packages/public/vibe-kanban-screenshot-workspace.png)

## Démarrage rapide

Authentifiez-vous d'abord auprès de votre agent IA préféré, puis exécutez :

```bash
npx vibe-kanban
```

C'est tout. Vibe Kanban démarre un serveur local et ouvre automatiquement votre navigateur.

## Fonctionnement

### Concepts clés

| Concept | Description |
|---------|-------------|
| **Project (Projet)** | Un dépôt git sur votre machine locale |
| **Issue (Tâche)** | Une carte de tâche sur le tableau kanban (titre + description + priorité + tags) |
| **Workspace (Espace de travail)** | Environnement d'exécution isolé — git worktree + agent IA + serveur de développement optionnel |

### Flux de travail typique

1. **Créez un projet** — connectez Vibe Kanban à un dépôt git local
2. **Ajoutez des issues** — décrivez le travail à faire sur le tableau kanban
3. **Démarrez un Workspace** — choisissez l'agent, la branche et les scripts de configuration/nettoyage ; le git worktree est créé automatiquement
4. **Observez l'agent travailler** — streaming de logs en temps réel dans la vue Workspace
5. **Révisez le diff** — vue unifiée ou côte à côte avec commentaires au niveau de la ligne
6. **Itérez** — soumettez vos commentaires de révision ; l'agent les lit et continue
7. **Livrez** — créez une PR avec une description générée par IA, révisez sur GitHub et fusionnez

## Agents de codage supportés

| Agent | Fournisseur |
|-------|-------------|
| Claude Code | Anthropic |
| OpenAI Codex CLI | OpenAI |
| Gemini CLI | Google |
| GitHub Copilot CLI | GitHub |
| Amp | Sourcegraph |
| Cursor Agent CLI | Anysphere |
| OpenCode | SST |
| Factory Droid | Factory AI |
| Claude Code Router (CCR) | Communauté |
| Qwen Code | Alibaba |

Consultez la [documentation officielle](https://vibekanban.com/docs/supported-coding-agents) pour les instructions d'installation et d'authentification de chaque agent.

## Serveur MCP

Vibe Kanban expose un serveur [MCP (Model Context Protocol)](https://modelcontextprotocol.io/) local, permettant à des clients externes (Claude Desktop, Raycast, etc.) de gérer les issues et Workspaces de manière programmatique.

```bash
# Démarrer le serveur MCP
npx vibe-kanban --mcp
```

Ou ajoutez-le à la configuration MCP de votre agent :

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

## Référence CLI

```bash
npx vibe-kanban               # Démarrer l'UI locale (par défaut)
npx vibe-kanban --mcp         # Démarrer le serveur MCP stdio
npx vibe-kanban review        # Exécuter la CLI de révision de code
npx vibe-kanban --help
npx vibe-kanban --version
```

## Documentation

Consultez le [site officiel](https://vibekanban.com/docs) pour la documentation complète et les guides utilisateur.

## Auto-hébergement

Vous souhaitez héberger votre propre instance Vibe Kanban Cloud ? Consultez le [guide d'auto-hébergement](https://vibekanban.com/docs/self-hosting/deploy-docker).

## Support

Utilisez [GitHub Discussions](https://github.com/BloopAI/vibe-kanban/discussions) pour les demandes de fonctionnalités et [GitHub Issues](https://github.com/BloopAI/vibe-kanban/issues) pour les bugs.

## Contributions

Veuillez ouvrir une [GitHub Discussion](https://github.com/BloopAI/vibe-kanban/discussions) ou rejoindre [Discord](https://discord.gg/AC4nwVtJM3) avant de soumettre une PR afin d'aligner les détails d'implémentation avec la feuille de route existante.

---

## Développement

### Prérequis

- [Rust](https://rustup.rs/) (dernière version stable)
- [Node.js](https://nodejs.org/) (≥ 20)
- [pnpm](https://pnpm.io/) (≥ 8)

```bash
cargo install cargo-watch
cargo install sqlx-cli
pnpm i
```

### Démarrer le serveur de développement

```bash
pnpm run dev
```

Démarre le backend Rust (rechargement à chaud via `cargo-watch`) et le serveur de développement Vite du frontend de manière concurrente. Au premier démarrage, une base de données SQLite vide est copiée depuis `dev_assets_seed/`.

### Compiler uniquement le frontend

```bash
cd packages/local-web
pnpm run build
```

### Compiler depuis les sources (génère le paquet npx-cli)

```bash
./local-build.sh
# Tester le résultat :
cd npx-cli && node bin/cli.js
```

### Vérification de types et linting

```bash
pnpm run check   # TypeScript (tous les packages) + Rust cargo check
pnpm run lint    # ESLint + cargo clippy
pnpm run format  # Prettier + cargo fmt
```

### Régénérer les types TypeScript partagés

```bash
pnpm run generate-types
```

Les types sont dérivés de structures Rust via [ts-rs](https://github.com/Aleph-Alpha/ts-rs). **Ne modifiez pas** `shared/types.ts` directement — modifiez `crates/server/src/bin/generate_types.rs`.

### Variables d'environnement

| Variable | Moment | Défaut | Description |
|----------|--------|--------|-------------|
| `PORT` | Exécution | Auto | Port du serveur en production. En développement : port du frontend (backend = PORT+1) |
| `FRONTEND_PORT` | Exécution | `3000` | Port Vite en mode développement |
| `BACKEND_PORT` | Exécution | `0` (auto) | Port du backend en mode développement |
| `HOST` | Exécution | `127.0.0.1` | Adresse de liaison du backend |
| `VK_ALLOWED_ORIGINS` | Exécution | — | Origines autorisées (séparées par des virgules), obligatoire derrière un proxy inverse |
| `DISABLE_WORKTREE_CLEANUP` | Exécution | — | Désactive le nettoyage automatique des git worktrees (pour le débogage) |
| `POSTHOG_API_KEY` | Compilation | — | Clé d'analytique PostHog (désactive les analytiques si vide) |

#### Derrière un proxy inverse

Configurez `VK_ALLOWED_ORIGINS` avec l'URL d'origine complète de votre frontend pour éviter les erreurs `403 Forbidden` :

```bash
VK_ALLOWED_ORIGINS=https://vk.example.com npx vibe-kanban
# Origines multiples :
VK_ALLOWED_ORIGINS=https://vk.example.com,https://vk-staging.example.com npx vibe-kanban
```

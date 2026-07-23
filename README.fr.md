<p align="center">
  <a href="https://github.com/magele758/hyper-vibekanban">
    <picture>
      <source srcset="packages/public/vibe-kanban-logo-dark.svg" media="(prefers-color-scheme: dark)">
      <source srcset="packages/public/vibe-kanban-logo.svg" media="(prefers-color-scheme: light)">
      <img src="packages/public/vibe-kanban-logo.svg" alt="Vibe Kanban Logo">
    </picture>
  </a>
</p>

<p align="center">Tirez 10× plus de Claude Code, Gemini CLI, Codex, Cursor, Pi et des autres agents de code...</p>
<p align="center">
  <a href="https://www.npmjs.com/package/vibe-kanban"><img alt="npm" src="https://img.shields.io/npm/v/vibe-kanban?style=flat-square" /></a>
  <a href="https://github.com/magele758/hyper-vibekanban/blob/main/.github/workflows/publish.yml"><img alt="Build status" src="https://img.shields.io/github/actions/workflow/status/magele758/hyper-vibekanban/.github%2Fworkflows%2Fpublish.yml" /></a>
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-Hans.md">简体中文</a> |
  <a href="README.zh-Hant.md">繁體中文</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.ko.md">한국어</a> |
  <a href="README.es.md">Español</a>
</p>

> **Note :** Le cloud officiel Vibe Kanban a été arrêté. Ce dépôt est un fork de [BloopAI/vibe-kanban](https://github.com/BloopAI/vibe-kanban) (hyper-vibekanban) : **toutes les capacités upstream sont conservées**, plus une nouvelle couche **board-agent dynamique** et un Remote auto-hébergé.

![](packages/public/screenshots/hyper-board.png)

## vs upstream : ce que l’on conserve / ce que l’on ajoute

L’upstream est un solide atelier d’agents + kanban fondé sur « ouvrir un Workspace à la main ». Ce fork le conserve tel quel et ajoute **événement de board → mise en file automatique → exécution → écriture en retour**, plus un remplacement auto-hébergé du cloud retiré.

### ✅ Hérité de l’upstream (entièrement conservé)

| Capacité | Description |
|------------|------------|
| **Kanban issues** | Création / priorité / tags / sous-issues / Team·Personal |
| **Workspace + git worktree** | Choisir un agent, worktree isolé, flux de logs en direct |
| **Sessions & follow-ups** | Chat multi-session, pièces jointes, @-files |
| **Inline diff review** | Unified / side-by-side ; les commentaires reviennent à l’agent |
| **App preview** | Navigateur intégré, DevTools, inspect, émulation d’appareils |
| **Coding agents** | Claude Code, Codex, Gemini, Copilot, Amp, Cursor, OpenCode, Droid, CCR, Qwen |
| **Git / PRs** | Rebase, UX de conflits, descriptions de PR par IA, merge GitHub / Azure |
| **MCP + Review CLI** | `npx vibe-kanban --mcp` / `review` |
| **Settings** | Agent profiles, MCP, intégration éditeur, notifications, org / projects |

Le chemin classique fonctionne toujours : **issue → ouvrir un Workspace à la main → logs → revue de diff → PR**.

### ✨ Ajouté dans ce fork (absent de l’upstream)

| Capacité | Description |
|------------|------------|
| **Board Agents** | Les agents sont de première classe sur le board ; **assign → enqueue** ; un watcher local ouvre un Workspace et écrit le progrès / les commentaires en retour |
| **Project Copilot** | Chat côté board (Cursor SDK par défaut) pour clarifier le travail et suggérer des assignations — **pas** l’exécuteur de code qui modifie les fichiers |
| **Squad DAG** | Pipelines multi-agents : Fork / Join / If / While ; éditeur canvas ; chat-to-pipeline optionnel |
| **Autopilot** | Cron + fuseau horaire ; créer des issues ou lancer un agent / squad ; skip / queue de concurrence |
| **Webhooks** | POST externe → créer une issue / mettre le travail en file |
| **Feishu bot** | Message Feishu → file d’issues ; réponse optionnelle une fois terminé |
| **Console workspaces** | Exécuter dans le **dir / branch courant** du dépôt sans forcer un nouveau worktree |
| **Host picker on create** | Lancer un workspace sur cette machine ou un remote worker appairé |
| **Mobile board layout** | Colonne unique + pills de statut pour téléphones |
| **Pi coding agent** | Pi CLI comme exécuteur Workspace supplémentaire |
| **Self-hosted Remote stack** | Docker Remote + Relay + ElectricSQL après l’arrêt du cloud (`scripts/vk-*.sh`) |

### 🔄 Amélioré par rapport à l’upstream

| Domaine | Upstream | Ce fork |
|------|----------|-----------|
| Remote Access | Appairage cloud officiel | Remote / Relay **self-hosted** ; SOP worker-host |
| Board | Cartes statiques + Workspace manuel | **Agents / squads assignables** avec écriture du progrès |
| Triggers | UI / MCP créent un Workspace | Aussi : assign, @, Autopilot, webhook, Feishu |

---

## Démonstration des fonctionnalités

Données de démo uniquement (Demo Org / Demo Showcase). Marqué **[Nouveau]** / **[Hérité]**.

### 1. [Nouveau] Board dynamique + Board Agents

Assignez un agent sur le board ; l’exécution est mise en file automatiquement et écrite en retour.

![](packages/public/screenshots/hyper-board.png)

![](packages/public/screenshots/hyper-agents.png)

### 2. [Nouveau] Project Copilot

Couche de chat/orchestration pour clarifier le travail ; le code se fait toujours dans les exécuteurs Workspace.

![](packages/public/screenshots/hyper-copilot.png)

### 3. [Nouveau] Pipelines Squad (DAG)

Plan → Fork → Implement / Review → Join ; créer depuis le chat, affiner sur le canvas.

![](packages/public/screenshots/hyper-squad.png)

![](packages/public/screenshots/hyper-squad-canvas.png)

### 4. [Nouveau] Autopilot / Webhooks / Feishu

Trois points d’entrée supplémentaires qui aboutissent tous à « créer une issue → mettre en file ».

![](packages/public/screenshots/hyper-autopilot.png)

![](packages/public/screenshots/hyper-webhooks.png)

![](packages/public/screenshots/hyper-feishu.png)

### 5. [Nouveau] Console workspace + host picker

- **Isolated worktree (hérité, par défaut)** — branch / dir dédiés
- **Console (nouveau)** — dir / branch courant ; pas de branch / commit auto
- **Execution host (nouveau)** — cette machine ou un remote worker appairé

![](packages/public/screenshots/hyper-create-console.png)

![](packages/public/screenshots/hyper-remote-access.png)

### 6. [Nouveau] Mise en page mobile du board

![](packages/public/screenshots/hyper-mobile-board.png)

### 7. [Hérité] Workspace sessions / diffs / preview

Cœur de l’upstream, conservé et peaufiné.

![](packages/public/screenshots/hyper-sessions.png)

![](packages/public/screenshots/hyper-diffs.png)

![](packages/public/screenshots/hyper-preview.png)

---

## Démarrage rapide

Authentifiez-vous d’abord avec votre agent de code préféré, puis :

```bash
npx vibe-kanban
```

Cela démarre le serveur local et ouvre le navigateur.

### Remote auto-hébergé (optionnel)

Après l’arrêt du cloud officiel, ce dépôt fournit une stack Docker Remote + Relay + ElectricSQL pour la synchro multi-appareils. Les helpers de développement sont sous `scripts/vk-*.sh` (ports dans `scripts/vk-ports.sh`). Voir le [guide d’auto-hébergement](docs/self-hosting/deploy-docker.mdx).

---

## Fonctionnement

### Concepts clés

| Concept | Description |
|---------|------------|
| **Project** | Un projet kanban (peut lier plusieurs dépôts git locaux) |
| **Issue** | Une carte de tâche sur le board |
| **Workspace** | Environnement d’exécution : worktree ou Console + coding agent |
| **Board Agent** | Rôle de chat assignable ; l’exécution réutilise les workspaces |
| **Squad** | Pipeline multi-agents + DAG |
| **Host** | Machine qui exécute réellement les agents (locale ou appairée) |

### Deux workflows

**A. Flux upstream (hérité, toujours entièrement pris en charge)**

1. Créer une issue → ouvrir un Workspace manuellement  
2. Suivre les logs / Preview → revoir les diffs → itérer  
3. Ouvrir une PR et merger  

**B. Flux board dynamique (nouveau dans ce fork)**

1. Créer un Board Agent (persona + executor par défaut)  
2. Assigner l’issue (ou déclencher via @ / webhook / Feishu / Autopilot)  
3. Le watcher local met le travail en file et ouvre un Workspace  
4. Le progrès / les commentaires sont écrits en retour ; clarifier éventuellement avec Copilot  
5. Orchestrer le travail multi-rôles avec un canvas Squad  
6. Revoir les diffs → ouvrir une PR (comme l’upstream)

---

## Agents de code pris en charge

| Agent | Provider |
|-------|----------|
| Claude Code | Anthropic |
| OpenAI Codex CLI | OpenAI |
| Gemini CLI | Google |
| GitHub Copilot CLI | GitHub |
| Amp | Sourcegraph |
| Cursor Agent CLI | Anysphere |
| OpenCode | SST |
| Factory Droid | Factory AI |
| Claude Code Router (CCR) | Community |
| Qwen Code | Alibaba |
| Pi | Pi (**ajouté dans ce fork**) |

Voir [supported coding agents](docs/supported-coding-agents.mdx). Les runtimes de chat du board (Copilot / agent chat) forment une couche distincte de ces exécuteurs de code — chat/orchestration est nouveau dans ce fork ; les exécuteurs de code sont la couche d’exécution upstream.

---

## MCP Server

```bash
npx vibe-kanban --mcp
```

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
npx vibe-kanban               # Local UI
npx vibe-kanban --mcp         # MCP stdio
npx vibe-kanban review        # Review CLI
npx vibe-kanban --help
```

## Documentation

- [`docs/`](docs/) — docs utilisateur + auto-hébergement
- [`docs/board-agents-plan.md`](docs/board-agents-plan.md) — conception board-agent
- [`docs/remote-access.mdx`](docs/remote-access.mdx) — remote access / pairing

## Support et contribution

Utilisez [Discussions](https://github.com/magele758/hyper-vibekanban/discussions) pour les idées et [Issues](https://github.com/magele758/hyper-vibekanban/issues) pour les bugs. Ouvrez une Discussion avant les grosses PR.

---

## Développement

### Prérequis

- [Rust](https://rustup.rs/) (dernière stable)
- [Node.js](https://nodejs.org/) (≥ 20)
- [pnpm](https://pnpm.io/) (≥ 8)

```bash
cargo install cargo-watch
cargo install sqlx-cli
pnpm i
```

### Serveur de développement

```bash
pnpm run dev
```

Démarre le backend Rust (`cargo-watch`) et Vite. Une SQLite DB vide est copiée depuis `dev_assets_seed/` au premier lancement.

Stack locale complète (Remote Docker + Relay + Desktop) :

```bash
bash scripts/vk-start.sh
bash scripts/vk-status.sh
```

### Compiler le paquet npx depuis les sources

```bash
./local-build.sh
cd npx-cli && node bin/cli.js
```

### Contrôles et types

```bash
pnpm run check
pnpm run lint
pnpm run format
pnpm run generate-types   # do not edit shared/types.ts by hand
```

### Variables d’environnement courantes

| Variable | Description |
|----------|-------------|
| `FRONTEND_PORT` / `BACKEND_PORT` / `HOST` | Ports de développement / bind |
| `VK_ALLOWED_ORIGINS` | Origins autorisés derrière un reverse proxy |
| `VK_SHARED_API_BASE` | Remote API (le serveur doit utiliser http) |
| `VK_SHARED_RELAY_API_BASE` | Relay API |
| `VK_TUNNEL` | Activer le mode tunnel du relay |

Définissez `VK_ALLOWED_ORIGINS` en reverse proxy, sinon le backend renvoie `403`. L’intégration éditeur Remote SSH est sous **Settings → Editor Integration**.

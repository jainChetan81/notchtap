# notchtap architecture diagrams

Rendered from `docs/ARCHITECTURE.md` (§ references below). Mermaid source —
previews in VS Code (`bierner.markdown-mermaid`), Zed (any Mermaid preview
extension), and directly on GitHub. draw.io can import Mermaid via
Extras → Edit Diagram → Insert → Mermaid if a drawn/edited version is needed.

## 1. system overview (§1)

```mermaid
flowchart TB
    subgraph inputs["input sources"]
        CLI["notchtap CLI<br/>(shell script: jq + curl)"]
        ADAPT["Agent Adapters<br/>(Claude Code · Codex · Kimi: notchtap-agent binary<br/>OpenCode: TS plugin)"]
        POLL["internal pollers<br/>(ESPN football · RSS news)"]
        TEST["settings test events<br/>(same paths as /notify · /agent/events)"]
    end

    subgraph core["rust core (src-tauri/)"]
        HTTP["loopback HTTP listener<br/>127.0.0.1:9789<br/>POST /notify · POST /agent/events"]
        VALID["validation + idempotency<br/>(eventId/sequence LRU, caps, sanitization)"]
        ENGINE["Engine (engine.rs)<br/>typed event bus · single-slot priority queue<br/>promotion · rotation · preemption · Topic supersession"]
        SILENCE["silence.rs<br/>Paused (absolute) · Silenced (High = Breakthrough)"]
        REGISTRY["Agent Registry (agents/registry.rs)<br/>live + retained sessions"]
        POLLERS["poller.rs · rss_poller.rs<br/>crests.rs · news_charge.rs"]
        TRAY["tray.rs · prefix.rs · global hotkeys"]
        CONFIG["config.rs<br/>~/.config/notchtap/config.toml<br/>(read once, restart = reload)"]
    end

    subgraph detect["runtime presentation-mode decision"]
        SHIM["notchtap-detect (Swift CLI)<br/>prints safe-area JSON"]
        DECIDE["presentation_mode(safe_area_top)<br/>Notch | HUD"]
    end

    subgraph ui["presentation ui (react/ts webview)"]
        OVERLAY["overlay window 'main' (index.html → App.tsx)<br/>receive-only · listen/unlisten only<br/>Notch mode (macbook) | HUD mode (mac mini)"]
        SETTINGS["settings window (settings.html → src/settings/)<br/>the only window allowed to invoke"]
        AGENTBOARD["Agent Board (idle surface,<br/>reads from rust-published registry state)"]
    end

    CLI -->|"flags-only HTTP"| HTTP
    ADAPT -->|"normalized schema v1, fail-open"| HTTP
    POLL -->|"deltas only"| POLLERS --> ENGINE
    TEST --> HTTP
    HTTP --> VALID --> ENGINE
    ENGINE <--> SILENCE
    ENGINE -->|"noteworthy agent events"| OVERLAY
    REGISTRY -->|"session lifecycle events"| AGENTBOARD
    SETTINGS -->|"tauri commands (allowlisted)"| CONFIG
    SETTINGS -->|"test events"| HTTP
    SHIM --> DECIDE -->|"window placement"| OVERLAY
    ENGINE -->|"emit SlotState"| OVERLAY
```

## 2. queue model — single slot, priority tiers, rotation (§6–7)

```mermaid
flowchart TB
    subgraph accept["acceptance (HTTP handlers)"]
        IN["push in"] -->|"tier cap exceeded"| R429["429"]
        IN --> OK["accepted (202)"]
    end

    OK --> LOW["Waiting line · Low"]
    OK --> MED["Waiting line · Medium"]
    OK --> HIGH["Waiting line · High"]

    subgraph engine["Engine — the only mutator of Slot/Waiting"]
        PROMOTE["promotion<br/>1. highest non-empty tier<br/>2. Rotation Order rank over Origin<br/>3. arrival order"]
        ROTATE["rotation<br/>Recurring → requeue to own tier<br/>OneShot → drop<br/>Topic → update card in place"]
        PREEMPT["preemption<br/>strictly-higher priority cuts visible card<br/>requeues at head of its tier with remaining turn<br/>equal priority never preempts"]
    end

    LOW & MED & HIGH --> PROMOTE --> SLOT["Slot — exactly one visible item"]
    SLOT --> ROTATE --> LOW & MED & HIGH
    PREEMPT --> SLOT

    PAUSE["Paused — promotion disabled,<br/>nothing dropped"] -.->|"gates"| PROMOTE
    SILENCE["Silenced — Medium/Low buffer,<br/>High promotes as Breakthrough (compact)"] -.->|"gates"| PROMOTE
```

## 3. ipc & security model — two windows, two trust levels (§9)

```mermaid
flowchart LR
    subgraph overlay["overlay window 'main' — receive-only, permanently"]
        FE["React frontend<br/>src/App.tsx, src/hooks/, src/components/"]
    end
    subgraph rustside["rust side"]
        EMIT["typed events down<br/>hover-changed · tab-selection-changed<br/>slot-state · agent events"]
        CLICK["NSEvent monitor (click.rs)<br/>clicks observed rust-side,<br/>pushed down the same channel"]
        HOVER["tracking area (hover.rs)"]
    end
    subgraph settingswin["settings window — the one exception"]
        SUI["src/settings/"] -->|"invoke (allowlisted commands,<br/>window-label checked)"| HANDLERS["settings_commands.rs<br/>build.rs deny-by-default list"]
    end

    FE -->|"listen/unlisten ONLY<br/>(capabilities/default.json, never change)"| EMIT
    EMIT --> FE
    CLICK --> EMIT
    HOVER --> EMIT
    HANDLERS --> CONFIGW[("config.toml atomic write<br/>→ relaunch")]
    HANDLERS --> APPEAR["set_appearance →<br/>appearance-changed (the one live-apply path)"]

    NOTE["three required legs:<br/>1. build.rs command allowlist (deny-by-default)<br/>2. capabilities/settings.json scoped to 'settings' window<br/>3. ensure_settings_window label check in every handler"]
```

## 4. agent integration (§10)

```mermaid
flowchart LR
    subgraph runtimes["agent runtimes"]
        CC["Claude Code hooks"]
        CX["Codex hooks"]
        KM["Kimi hooks"]
        OC["OpenCode plugin bus"]
    end
    subgraph delivery["adapter delivery — fail-open observers"]
        BIN["notchtap-agent binary<br/>hook/test/status/doctor<br/>≤750ms timeout, exit 0 on failure"]
        TS["adapters/opencode/notchtap.ts"]
    end
    ENDPOINT["POST /agent/events<br/>schema v1 · identity = runtime + session id<br/>idempotent on eventId/sequence"]
    REG["Agent Registry<br/>session states, urgency ordering,<br/>10-min terminal retention"]
    NOTIF["noteworthy kinds → queue/Slot<br/>(permission · input · failure = High)"]
    BOARD["Agent Board — idle surface<br/>when Slot is empty"]
    SETTINGS_UI["Settings: Adapter Health,<br/>doctor read-only, setup snippets"]

    CC & CX & KM --> BIN --> ENDPOINT
    OC --> TS --> ENDPOINT
    ENDPOINT --> REG
    REG --> NOTIF
    REG --> BOARD
    REG --> SETTINGS_UI
    BOUNDARY["heads-up only: never approves, replies,<br/>launches, supervises, or scrapes the runtime"]
```

## 5. frontend composition (overlay)

```mermaid
flowchart TB
    subgraph App["App.tsx — receives only"]
        HOOKS["hooks (one per file, src/hooks/)<br/>useSlotState · useStatusState · useAgentState<br/>useTabSelection · useAgentViewedSession<br/>useClock · useDelayedSwap · useExitChoreography"]
        MODE["presentationMode()<br/>rail | notch | hud · agent board vs card"]
        subgraph surfaces
            CARD["StatusRailCard / NotificationBody<br/>TtlBar · Stamp · FlankClock"]
            IDLE["IdleFace · AgentBoard · IconStrip<br/>IdleHoverPeek · TabBelowBlock · NewsBelowBlock<br/>AgentBelowBlock · StatusDots"]
        end
        LIB["src/lib/ pure helpers<br/>presentation · presentationFacts · format<br/>guards · iconPresence · sourceColors"]
    end
    BUS["rust event bus"] -->|"listen()"| HOOKS --> MODE --> surfaces
    LIB --- surfaces
```

## 6. cross-device behaviour (§2–3)

```mermaid
flowchart TB
    SAME["same compiled app · same rust core · same react ui"]
    SAME --> CHECK{"notchtap-detect:<br/>safeAreaInsets.top > 0?"}
    CHECK -->|"yes (macbook)"| NOTCH["Notch mode — window pinned<br/>flush over the notch cutout"]
    CHECK -->|"no (mac mini)"| HUD["HUD mode — floating<br/>top-center, synthetic cutout"]
    NOTCH & HUD --> IDENTICAL["everything else identical:<br/>queue · animation · cli input · agent board"]
```

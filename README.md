> **2026-10-21 native build, security, UI/UX & acceptance verified:** Upstream `storytold/effectcraft@de3fec28ed7a3d7a700a9d8ea307779dcda99a8e` is merged with full Git ancestry and verified on Windows 11 x64 (`wgpu` DX12 + CPU fallback). Native control authentication (`ControlAuthToken` + RAII `ConnectionSlot`), **Inferno Pixel Glass** Dark & Light themes (`gemi-core.v1.json`), `GemiFontSans` + Arabic RTL shaping, GemiMCP product manifest (`contracts/gemimcp-product-aftertron.v1.json`, 315 tools), OIDC PKCE (`auth.gemitron.net`), and GemiGuard Ed25519 offline lease verification are implemented and verified. See [security audit](docs/implementation/07-NATIVE-CONTROL-AND-MCP-SECURITY-AUDIT.md), [completeness & parity matrix](docs/implementation/08-SOURCE-COMPLETENESS-AND-PARITY-MATRIX.md), [original upstream README](docs/upstream/ORIGINAL-README.md), and [source receipt](docs/upstream/IMPORT-RECORD.txt).

# AfterTron — Independent Native Motion Graphics & VFX Compositor

**Product:** Professional After Effects-class compositor, motion graphics, and visual effects desktop application in pure Rust.  
**Canonical repo:** [`Elwazeery/AfterTron`](https://github.com/Elwazeery/AfterTron)  
**Approved upstream:** [`storytold/effectcraft`](https://github.com/storytold/effectcraft) — preserves full upstream history, MIT OR Apache-2.0 dual licensing, `ATTRIBUTION.md`, and `.attribution` sidecars.  
**Creative product independence:** Independent of GemiMotion, PhotoTron, and FilmTron. No merged editing engines or duplicated cloud MCP server runtimes.  
**Unified Cloud MCP:** [`GemiTron88/GemiMCP`](https://github.com/GemiTron88/GemiMCP) is the sole platform cloud MCP runtime owner (`https://mcp.gemitron.net/products/after/mcp`). AfterTron publishes its versioned capability & tool manifest at [`contracts/gemimcp-product-aftertron.v1.json`](contracts/gemimcp-product-aftertron.v1.json) and bridges commands via `OutboundBridgeAdapter` (`crates/automation/src/gemimcp.rs`).

## Visual Preview — Inferno Pixel Glass UI

| Inferno Pixel Glass Dark (`inferno-dark`) | Inferno Pixel Glass Light (`inferno-light`) |
|---|---|
| ![AfterTron Inferno Dark Hero](docs/images/aftertron/aftertron-inferno-dark-hero.png) | ![AfterTron Inferno Light Workspace](docs/images/aftertron/aftertron-inferno-light-workspace.png) |

| Graph Editor & Effect Controls | Render Queue & Lumetri Scopes |
|---|---|
| ![AfterTron Graph Editor & Effects](docs/images/aftertron/aftertron-graph-editor-effects.png) | ![AfterTron Render Queue & Scopes](docs/images/aftertron/aftertron-render-queue-and-scopes.png) |

## Verified Architecture & Capabilities

- **Exact Flick Timebase & Compositing (`crates/time`, `crates/render`, `crates/gpu`):** Frame-exact `Tick` ($705{,}600{,}000\text{ ticks/s}$) timing, 8/16/32-bpc compositing, 38 blend modes, Classic 3D & Advanced 3D ray-marched renderer, motion blur, track mattes, and feather/expansion masks with `wgpu` GPU acceleration and automatic CPU fallback (`GpuFailureBridge`).
- **Animation, Expressions & 213 Built-In Effects (`crates/keyframe`, `crates/expr`, `crates/effects`):** Linear, Hold, and Cubic Bezier spatial/temporal interpolation, Easy Ease, Value & Speed Graph Editor, sandboxed JavaScript expressions (`boa_engine`), and 213 effects across 19 categories plus WebAssembly (`wasmi`) plugins.
- **Typography & Arabic RTL (`crates/text`):** Bundled `GemiFontSans` (Light, Medium, Bold), `Inter`, `JetBrains Mono`, and `Noto Serif` with `skrifa` + `harfrust` OpenType shaping, contextual Arabic joining, right-to-left glyph layout, and per-character text animators.
- **20-Preset `GemiTokens` & Neutral Compositing Surfaces (`crates/ui-egui/src/{gemi_tokens,theme}.rs`):** Full 20-preset / 40-mode `GemiTokens` synchronization (`inferno`, `royalGold`, `cyberCyan`, `liquidGlass`, `emeraldMushaf`, etc.) while locking the composition viewer pasteboard (`#181818`) and scopes (`#121212`) strictly to `R == G == B` color neutrality.
- **Unified AI Brain (`⚡ TRON AI · ✦ AGY AI · ☁️ GemiAI`, `crates/automation/src/antigravity_brain.rs`, `crates/ui-egui/src/tron_chat.rs`):** 14-model `AGYTron` Smart Mind catalog, 5 effort tiers (`xhigh` = 24,576 thinking tokens / 4 critique passes), `device-fleet` vs `account-remote-agent` execution scopes, live `:18220` `/v1/agy/smart-mind` + `/v1/agy/chat-stream` sync (`AGY_TOS_SAFETY_GUARD`), and 7 Egyptian Arabic VFX/motion feedback refinements with 1-click Undo.
- **Clean-Room `.aepx` (`705,600,000` Flicks/s), `troncs.handoff.v1` & `AfterTron-Mobile` (`crates/interchange/src/aepx.rs`, `crates/host/src/mobile.rs`):** Loss-disclosed `.aepx` XML composition/layer/keyframe interchange (`EXACT_VERIFIED`, `COMPATIBLE_WITH_TRANSFORM`, `LOSSY_APPROVAL_REQUIRED`), `PhotoTron -> AfterTron -> FilmTron` dynamic asset link refresh, and portable `AfterTron-Mobile` Vulkan/Metal/CPU capability adapter.
- **Authenticated Local Control & GemiMCP Parity (`apps/effectcraft/src/control_server.rs`, `crates/automation`):** Per-launch 256-bit CSPRNG hex token (`ControlAuthToken`), mandatory `auth` handshake within 10s deadline, loopback-only bind enforcement, owner-only token file ACL (`--control-token-out`), RAII `ConnectionSlot`, `AuthorizedWorkspace` path confinement, `expectedRevision` optimistic concurrency, and `idempotencyKey` replay protection.
- **Unified Identity & GemiGuard Offline Entitlements (`crates/automation/src/gemiguard.rs`):** OIDC PKCE (`S256`) authorization request builder for `https://auth.gemitron.net` and pure-Rust RFC 8032 Ed25519 offline lease & revocation list verifier that always preserves local `.ecproj` access (`preserve_user_project_data: true`, `allows_local_project_io: true`).

## Canonical Documentation

- [Product and complete MCP parity implementation charter](docs/01-FORK-PRODUCT-AND-CLOUD-MCP-CHARTER.md)
- [Native control & cloud MCP security audit](docs/implementation/07-NATIVE-CONTROL-AND-MCP-SECURITY-AUDIT.md)
- [Deep source completeness & parity matrix](docs/implementation/08-SOURCE-COMPLETENESS-AND-PARITY-MATRIX.md)
- [Execution task backlog](TODO.md)
- [Asset attribution index](ATTRIBUTION.md)

## 2026-10-21 — TRON optional local creative AI: design intake only

**Scope amendment (owner-approved; implementation NOT STARTED):** [TRON hybrid design](docs/creative-ai/TRON-LOCAL-CREATIVE-INTEGRATION-20261021.md); [TRON Egyptian Gen Z persona/training](https://github.com/GemiTron88/GemiMCP/blob/main/docs/creative-ai/TRON-PERSONA-AND-TRAINING-STRATEGY.v1.md); [shared creative rules](https://github.com/GemiTron88/GemiMCP/blob/main/docs/creative-ai/CREATIVE-KNOWLEDGE-AND-WORKFLOWS.v1.md); [optional ChatGPT->TRON delegation](https://github.com/GemiTron88/GemiMCP/blob/main/contracts/CREATIVE-AI-DELEGATION.v1.md). This specifically amends older no-local-AI statements while retaining ChatGPT-first, independent native editors, ONLY GemiMCP cloud MCP server, Gemi Access/GemiGuard, no paid model API and no mandatory downloads. One shared AI runtime, Qwen3.5-4B quant **candidate for inference** only; no Qwen3.5-4B/6GB-QLoRA feasibility claim. Closing last tab/job lease unloads GPU weights; other app leases must not be interrupted. **No model installed, trained, measured or deployed from this change.**

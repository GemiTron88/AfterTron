//! AfterTron × TRON native floating motion director.
//! Local AI is optional, OFF on launch, shared with PhotoTron/FilmTron.
//! This chat never creates a second model/runtime and never calls shell or
//! submits model output straight to the native engine without user approval.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use egui::{
    Align, Align2, Color32, Context, CornerRadius, Frame, Id, Layout, Margin, Order, RichText,
    ScrollArea, Stroke, vec2,
};
use serde_json::{Value, json};

use effectcraft_automation::antigravity_brain::{
    AGY_REMOTE_AGENT_RELAY_URL, AntigravityAuthMode, AntigravityDirectOutcome,
    AntigravityDirectRequest, AntigravityEffortLevel, AntigravityExecutionScope,
    AntigravityFleetNodeId, AntigravityModelId, AntigravityVibePreset, ArabicMotionFeedback,
    FleetAddressBook, FleetNodeLiveStatus, STUDIO_OWNER_CONTROL_HUB_URL,
    apply_arabic_motion_feedback_session, direct_with_antigravity_brain_session,
    probe_fleet_node_live_status, sync_smart_mind_catalog_from_daemon,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DualBrainMode {
    #[default]
    TronAi,
    AgyAi,
    GemiAi,
}

impl DualBrainMode {
    pub const ALL: [Self; 3] = [Self::TronAi, Self::AgyAi, Self::GemiAi];

    pub const fn label(self) -> &'static str {
        match self {
            Self::TronAi => "⚡ TRON AI",
            Self::AgyAi => "✦ AGY AI",
            Self::GemiAi => "☁️ GemiAI",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ChatTurn {
    pub role: String,
    pub text: String,
    pub badge: Option<String>,
}

struct ChatState {
    open: bool,
    docked: bool,
    brain_mode: DualBrainMode,
    working: bool,
    task_label: String,
    power_on: bool,
    transport_connected: bool,
    model_state: String,
    model: String,
    thinking: bool,
    warmed: bool,
    leases: u64,
    vram_used_mb: Option<u64>,
    vram_total_mb: Option<u64>,
    error: Option<String>,
    prompt: String,
    selected_direction: usize,
    proposal: Option<Value>,
    reviewed_id: Option<String>,
    reviewed_revision: Option<u64>,
    reviewed_digest: Option<String>,
    review: Option<Value>,
    execution: Option<Value>,
    rendered_samples: Vec<Value>,
    critique: Option<Value>,
    history: Vec<ChatTurn>,
    capture_mode: Option<String>,
    last_status: Instant,
    pub agy_execution_scope: AntigravityExecutionScope,
    agy_preset: AntigravityVibePreset,
    agy_model: AntigravityModelId,
    agy_effort: AntigravityEffortLevel,
    agy_node: AntigravityFleetNodeId,
    agy_fleet_ips: FleetAddressBook,
    agy_fleet_status: Option<FleetNodeLiveStatus>,
    agy_outcome: Option<AntigravityDirectOutcome>,
    gemi_preset: String,
    agy_daemon_sync_badge: String,
}

impl Default for ChatState {
    fn default() -> Self {
        let agy_preset = AntigravityVibePreset::default();
        Self {
            open: false,
            docked: false,
            brain_mode: DualBrainMode::default(),
            working: false,
            task_label: String::new(),
            power_on: false,
            transport_connected: false,
            model_state: "NOT_LOADED".into(),
            model: "qwen3.5:4b-q4_K_M".into(),
            thinking: false,
            warmed: false,
            leases: 0,
            vram_used_mb: None,
            vram_total_mb: None,
            error: None,
            prompt: agy_preset.default_prompt().to_string(),
            selected_direction: 0,
            proposal: None,
            reviewed_id: None,
            reviewed_revision: None,
            reviewed_digest: None,
            review: None,
            execution: None,
            rendered_samples: Vec::new(),
            critique: None,
            history: Vec::new(),
            capture_mode: None,
            last_status: Instant::now(),
            agy_execution_scope: AntigravityExecutionScope::default(),
            agy_preset,
            agy_model: AntigravityModelId::default(),
            agy_effort: AntigravityEffortLevel::default(),
            agy_node: AntigravityFleetNodeId::default(),
            agy_fleet_ips: FleetAddressBook::default(),
            agy_fleet_status: None,
            agy_outcome: None,
            gemi_preset: "inferno".into(),
            agy_daemon_sync_badge: "Standby · 14 Smart Mind Models".into(),
        }
    }
}

#[derive(Clone, Copy)]
struct InfernoGlassPalette {
    bg_primary: Color32,
    bg_card: Color32,
    bg_card_selected: Color32,
    border: Color32,
    border_active: Color32,
    accent_crimson: Color32,
    accent_gold: Color32,
    accent_emerald: Color32,
    text_primary: Color32,
    text_secondary: Color32,
    text_muted: Color32,
}

fn palette_for(dark: bool) -> InfernoGlassPalette {
    if dark {
        InfernoGlassPalette {
            bg_primary: Color32::from_rgb(0x0B, 0x0C, 0x10),
            bg_card: Color32::from_rgb(0x14, 0x16, 0x1D),
            bg_card_selected: Color32::from_rgb(0x1F, 0x17, 0x1D),
            border: Color32::from_rgb(0x2A, 0x2E, 0x3B),
            border_active: Color32::from_rgb(0xF8, 0x39, 0x42),
            accent_crimson: Color32::from_rgb(0xF8, 0x39, 0x42),
            accent_gold: Color32::from_rgb(0xFF, 0xD4, 0x37),
            accent_emerald: Color32::from_rgb(0x34, 0xD3, 0x99),
            text_primary: Color32::from_rgb(0xF5, 0xF1, 0xED),
            text_secondary: Color32::from_rgb(0xCB, 0xD5, 0xE1),
            text_muted: Color32::from_rgb(0x94, 0xA3, 0xB8),
        }
    } else {
        InfernoGlassPalette {
            bg_primary: Color32::from_rgb(0xF6, 0xF2, 0xEE),
            bg_card: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            bg_card_selected: Color32::from_rgb(0xFD, 0xF2, 0xF2),
            border: Color32::from_rgb(0xD8, 0xD2, 0xCA),
            border_active: Color32::from_rgb(0xDF, 0x1F, 0x2D),
            accent_crimson: Color32::from_rgb(0xDF, 0x1F, 0x2D),
            accent_gold: Color32::from_rgb(0xB4, 0x53, 0x09),
            accent_emerald: Color32::from_rgb(0x05, 0x96, 0x69),
            text_primary: Color32::from_rgb(0x15, 0x13, 0x18),
            text_secondary: Color32::from_rgb(0x33, 0x41, 0x55),
            text_muted: Color32::from_rgb(0x64, 0x74, 0x8B),
        }
    }
}

fn shared(ctx: &Context) -> Arc<Mutex<ChatState>> {
    let id = Id::new("aftertron.tron.local.view");
    ctx.data_mut(|data| match data.get_temp::<Arc<Mutex<ChatState>>>(id) {
        Some(state) => state,
        None => {
            let s = Arc::new(Mutex::new(ChatState::default()));
            data.insert_temp(id, s.clone());
            s
        }
    })
}

fn is_rtl_text(s: &str) -> bool {
    s.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c))
}

fn draw_bidi_paragraph(ui: &mut egui::Ui, text: &str, color: Color32, strong: bool) {
    let rtl = is_rtl_text(text);
    let align = if rtl { Align::Max } else { Align::Min };
    ui.with_layout(Layout::top_down(align), |ui| {
        let mut rich = RichText::new(text).color(color);
        if strong {
            rich = rich.strong();
        }
        ui.label(rich);
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn update_status(state: &mut ChatState, raw: &Value) {
    let status = raw.get("status").unwrap_or(raw);
    state.transport_connected = true;
    state.model_state = status
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("UNKNOWN")
        .to_owned();
    state.model = status
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("qwen3.5:4b-q4_K_M")
        .to_owned();
    state.thinking = status
        .get("thinkingVerified")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    state.warmed = status
        .get("warmupVerified")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    state.leases = status
        .get("activeLeaseCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if let Some(gpu) = status.get("gpu") {
        state.vram_used_mb = gpu
            .get("usedMiB")
            .or_else(|| gpu.get("usedMb"))
            .and_then(Value::as_u64);
        state.vram_total_mb = gpu
            .get("totalMiB")
            .or_else(|| gpu.get("totalMb"))
            .and_then(Value::as_u64);
    }
}

/// ALL broker I/O runs off the UI frame thread.
/// Cancellation and OFF are permitted even while `working` is true so active inference jobs
/// are never stranded.
#[cfg(not(target_arch = "wasm32"))]
fn action(st: Arc<Mutex<ChatState>>, ctx: Context, task: &'static str) {
    let Ok(mut state) = st.lock() else { return };
    let is_interrupt = matches!(task, "cancel" | "off" | "minimize");
    if state.working && !is_interrupt {
        return;
    }
    let was_working = state.working;
    state.working = true;
    state.task_label = match task {
        "on" => "جاري تشغيل محرك TRON والتحقق من التفكير...".into(),
        "off" => "جاري إيقاف الجلسة وتحرير الذاكرة...".into(),
        "cancel" => "جاري إلغاء المهمة الحالية...".into(),
        _ => String::new(),
    };
    state.error = None;
    drop(state);
    std::thread::spawn(move || {
        if was_working && matches!(task, "off" | "minimize") {
            let _ = effectcraft_automation::tron_local::cancel_job();
        }
        let reply = match task {
            "on" => effectcraft_automation::tron_local::activate(),
            "off" => effectcraft_automation::tron_local::release(),
            "minimize" => effectcraft_automation::tron_local::minimize(),
            "heartbeat" => effectcraft_automation::tron_local::heartbeat(),
            "cancel" => effectcraft_automation::tron_local::cancel_job(),
            _ => effectcraft_automation::tron_local::status(),
        };
        if let Ok(mut state) = st.lock() {
            state.working = false;
            state.task_label.clear();
            state.last_status = Instant::now();
            match reply {
                Ok(value) => {
                    update_status(&mut state, &value);
                    if task == "on" {
                        state.power_on =
                            state.thinking && state.warmed && state.model_state == "READY";
                        if !state.power_on {
                            state.error = Some("TRON reasoning/warm-up not verified".into());
                        }
                    }
                    if task == "heartbeat"
                        && value.get("alive").and_then(Value::as_bool) != Some(true)
                    {
                        state.power_on = false;
                        state.reviewed_id = None;
                        state.reviewed_revision = None;
                        state.reviewed_digest = None;
                        state.review = None;
                        state.error = Some("AFTERTRON_MODEL_LEASE_EXPIRED".into());
                    }
                    if matches!(task, "off" | "minimize") {
                        state.power_on = false;
                        state.review = None;
                        state.reviewed_id = None;
                        state.reviewed_revision = None;
                        state.reviewed_digest = None;
                    }
                }
                Err(e) => {
                    state.error = Some(e);
                    if task == "on" {
                        state.power_on = false;
                    }
                    if task == "status" {
                        state.transport_connected = false;
                    }
                }
            }
        }
        ctx.request_repaint();
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn generate(st: Arc<Mutex<ChatState>>, ctx: Context, prompt: String, context: Value) {
    let Ok(mut state) = st.lock() else { return };
    if state.working || !state.power_on {
        return;
    }
    state.working = true;
    state.task_label = "جاري تحليل الكومبوزيشن وبناء اتجاهين إبداعيين...".into();
    state.error = None;
    state.proposal = None;
    state.review = None;
    state.execution = None;
    state.critique = None;
    state.rendered_samples.clear();
    state.reviewed_id = None;
    state.reviewed_revision = None;
    state.reviewed_digest = None;
    state.selected_direction = 0;
    state.history.push(ChatTurn {
        role: "user".into(),
        text: prompt.clone(),
        badge: Some("Brief".into()),
    });
    drop(state);
    std::thread::spawn(move || {
        let reply = effectcraft_automation::tron_local::prepare_after_plan(&prompt, &context);
        if let Ok(mut state) = st.lock() {
            state.working = false;
            state.task_label.clear();
            state.last_status = Instant::now();
            match reply {
                Ok(data) => {
                    let result = data.get("result").unwrap_or(&data);
                    if result.get("ok").and_then(Value::as_bool) == Some(false) {
                        state.error = result
                            .get("status")
                            .and_then(Value::as_str)
                            .map(str::to_owned);
                    }
                    if let Some(expl) = result.get("userExplanation").and_then(Value::as_str) {
                        state.history.push(ChatTurn {
                            role: "tron".into(),
                            text: expl.to_owned(),
                            badge: Some("Art Direction · اتجاهين".into()),
                        });
                    }
                    state.proposal = Some(data);
                }
                Err(e) => state.error = Some(e),
            }
        }
        ctx.request_repaint();
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn run_visual_critique(
    st: Arc<Mutex<ChatState>>,
    ctx: Context,
    project_state: Value,
    samples: Vec<Value>,
    prompt: String,
) {
    let Ok(mut state) = st.lock() else { return };
    if state.working || !state.power_on {
        return;
    }
    state.working = true;
    state.task_label = "جاري فحص 5 كادرات حقيقية ومراجعة التباين والحركة...".into();
    state.error = None;
    state.rendered_samples = samples.clone();
    drop(state);
    std::thread::spawn(move || {
        let reply = effectcraft_automation::tron_local::critique_after_execution(
            &project_state,
            &samples,
            &prompt,
        );
        if let Ok(mut state) = st.lock() {
            state.working = false;
            state.task_label.clear();
            state.last_status = Instant::now();
            match reply {
                Ok(data) => {
                    let crit = data.get("critique").unwrap_or(&data);
                    if let Some(text) = crit.get("critiqueText").and_then(Value::as_str) {
                        state.history.push(ChatTurn {
                            role: "tron".into(),
                            text: text.to_owned(),
                            badge: Some("5-Frame Visual QC".into()),
                        });
                    }
                    state.critique = Some(data);
                }
                Err(e) => state.error = Some(e),
            }
        }
        ctx.request_repaint();
    });
}

/// Seed deterministic UI fixture state (`UI_FIXTURE`) for layout testing only.
/// For creative acceptance, use `bind_live_runtime_state_for_capture` (`LIVE_RUNTIME`).
pub fn seed_ui_fixture_state_for_capture(ctx: &Context, open: bool, power_on: bool) {
    seed_acceptance_state_for_capture(ctx, open, power_on);
}

/// Seed deterministic UI fixture state (`UI_FIXTURE`) for headless layout verification.
pub fn seed_acceptance_state_for_capture(ctx: &Context, open: bool, power_on: bool) {
    let st = shared(ctx);
    let Ok(mut s) = st.lock() else { return };
    s.open = open;
    s.docked = false;
    s.working = false;
    s.power_on = power_on;
    s.transport_connected = power_on;
    s.capture_mode = Some("UI_FIXTURE".into());
    s.last_status = Instant::now();
    s.model_state = if power_on {
        "READY".into()
    } else {
        "NOT_LOADED".into()
    };
    s.model = "qwen3.5:4b-q4_K_M".into();
    s.thinking = power_on;
    s.warmed = power_on;
    s.leases = if power_on { 1 } else { 0 };
    s.vram_used_mb = if power_on { Some(3120) } else { Some(410) };
    s.vram_total_mb = Some(6144);
    s.prompt = "صمّم تتر موشن تايبوجرافي احترافي بـ Hierarchy واضح وEasy Ease".into();
    s.selected_direction = 0;
    s.history = vec![
        ChatTurn {
            role: "user".into(),
            text: "صمّم تتر موشن تايبوجرافي احترافي بـ Hierarchy واضح وEasy Ease".into(),
            badge: Some("Brief · UI_FIXTURE".into()),
        },
        ChatTurn {
            role: "tron".into(),
            text: "[UI_FIXTURE] راجعنا الكومبوزيشن على 24fps وجهزنا اتجاهين مختلفين فعلاً في الإيقاع والتكوين البصري: الأول مبني على Kinetic Authority بـ Overshoot محسوب، والتاني Editorial Split بـ Matte نظيف.".into(),
            badge: Some("UI_FIXTURE · Art Direction".into()),
        },
    ];
    s.proposal = Some(json!({
        "ok": true,
        "result": {
            "ok": true,
            "captureMode": "UI_FIXTURE",
            "selectedDirectionIndex": 0,
            "selectedDirectionId": "direction_a",
            "userExplanation": "[UI_FIXTURE] اتجاهين إبداعيين جاهزين للمقارنة والاعتماد قبل أي تعديل على الـ Timeline.",
            "exploredDirections": [
                {
                    "id": "direction_a",
                    "label": "الاتجاه الأول — Kinetic Authority & Precision Rig",
                    "summary": "دخول متدرج للعناوين مع Null Rig مركزي ومنحنيات سرعة حادة في البداية وتهدئة ناعمة.",
                    "plan": {
                        "schema": "aftertron.tron.plan.v1",
                        "product": "aftertron",
                        "planId": "dir-a-plan",
                        "projectId": "aftertron-local-demo",
                        "compId": 1,
                        "expectedRevision": 1,
                        "fpsNum": 24,
                        "fpsDen": 1,
                        "steps": [
                            {"id": "s1", "tool": "after.layer.create_solid", "params": {"name": "BG_Obsidian", "color": "#090D16", "width": 1920, "height": 1080}},
                            {"id": "s2", "tool": "after.layer.create_null", "params": {"name": "Typography_Rig", "position": [960.0, 540.0]}},
                            {"id": "s3", "tool": "after.layer.create_text", "params": {"name": "Hero_Headline", "text": "AFTERTRON STUDIO", "fontSize": 92.0, "fillColor": "#F8FAFC"}},
                            {"id": "s4", "tool": "after.keyframe.add", "params": {"layer": "Hero_Headline", "path": "Transform/Position", "timeTick": 0, "value": [960.0, 620.0], "easyEase": true}}
                        ]
                    }
                },
                {
                    "id": "direction_b",
                    "label": "الاتجاه الثاني — Editorial Split & Luma Matte Reveal",
                    "summary": "تكوين تحريري متوازن مع كشف تدريجي عبر الماسكات وتباين لوني دافئ.",
                    "plan": {
                        "schema": "aftertron.tron.plan.v1",
                        "product": "aftertron",
                        "planId": "dir-b-plan",
                        "projectId": "aftertron-local-demo",
                        "compId": 1,
                        "expectedRevision": 1,
                        "fpsNum": 24,
                        "fpsDen": 1,
                        "steps": [
                            {"id": "b1", "tool": "after.layer.create_solid", "params": {"name": "BG_Editorial", "color": "#10141D", "width": 1920, "height": 1080}},
                            {"id": "b2", "tool": "after.layer.create_shape", "params": {"name": "Reveal_Frame", "shape": "rect", "size": [760.0, 180.0], "fillColor": "#F83942"}},
                            {"id": "b3", "tool": "after.layer.create_text", "params": {"name": "Editorial_Title", "text": "MOTION CRAFT", "fontSize": 84.0, "fillColor": "#F5F1ED"}}
                        ]
                    }
                }
            ],
            "plan": {
                "schema": "aftertron.tron.plan.v1",
                "product": "aftertron",
                "planId": "dir-a-plan",
                "projectId": "aftertron-local-demo",
                "compId": 1,
                "expectedRevision": 1,
                "fpsNum": 24,
                "fpsDen": 1,
                "steps": [
                    {"id": "s1", "tool": "after.layer.create_solid", "params": {"name": "BG_Obsidian", "color": "#090D16", "width": 1920, "height": 1080}},
                    {"id": "s2", "tool": "after.layer.create_null", "params": {"name": "Typography_Rig", "position": [960.0, 540.0]}},
                    {"id": "s3", "tool": "after.layer.create_text", "params": {"name": "Hero_Headline", "text": "AFTERTRON STUDIO", "fontSize": 92.0, "fillColor": "#F8FAFC"}},
                    {"id": "s4", "tool": "after.keyframe.add", "params": {"layer": "Hero_Headline", "path": "Transform/Position", "timeTick": 0, "value": [960.0, 620.0], "easyEase": true}}
                ]
            }
        }
    }));
    s.review = Some(json!({
        "status": "REVIEWED_NOT_EXECUTED",
        "planId": "dir-a-plan",
        "planDigest": "8f4a2c91e0b37d12",
        "revision": 1
    }));
}

/// Bind authentic live runtime outputs (`LIVE_RUNTIME`) from the shared GemiMCP broker,
/// live model inference, native preflight review, native execution receipt, rendered frame
/// samples, and multimodal vision critique into the TRON UI state for live acceptance capture.
#[allow(clippy::too_many_arguments)]
pub fn bind_live_runtime_state_for_capture(
    ctx: &Context,
    open: bool,
    docked: bool,
    live_status: &Value,
    prompt: &str,
    selected_direction: usize,
    live_proposal: Option<Value>,
    live_review: Option<Value>,
    live_execution: Option<Value>,
    rendered_samples: Vec<Value>,
    live_critique: Option<Value>,
) {
    let st = shared(ctx);
    let Ok(mut s) = st.lock() else { return };
    s.open = open;
    s.docked = docked;
    s.working = false;
    s.task_label.clear();
    s.error = None;
    s.capture_mode = Some("LIVE_RUNTIME".into());
    s.last_status = Instant::now();
    #[cfg(not(target_arch = "wasm32"))]
    update_status(&mut s, live_status);
    s.power_on = s.thinking && s.warmed && s.model_state == "READY";
    s.prompt = prompt.to_owned();
    s.selected_direction = selected_direction;
    s.history.clear();
    if !prompt.trim().is_empty() {
        s.history.push(ChatTurn {
            role: "user".into(),
            text: prompt.to_owned(),
            badge: Some("Brief · LIVE_RUNTIME".into()),
        });
    }
    if let Some(prop) = &live_proposal {
        let res = prop.get("result").unwrap_or(prop);
        if let Some(exp) = res
            .get("directorResponseAr")
            .or_else(|| res.get("userExplanation"))
            .and_then(Value::as_str)
        {
            s.history.push(ChatTurn {
                role: "tron".into(),
                text: exp.to_owned(),
                badge: Some("LIVE_RUNTIME · Model Plan".into()),
            });
        }
    }
    if let Some(crit) = &live_critique {
        let c = crit.get("critique").or_else(|| crit.get("result")).unwrap_or(crit);
        if let Some(txt) = c.get("critiqueText").or_else(|| c.get("userSummary")).and_then(Value::as_str) {
            s.history.push(ChatTurn {
                role: "tron".into(),
                text: txt.to_owned(),
                badge: Some("LIVE_RUNTIME · 5-Frame Vision QC".into()),
            });
        }
    }
    if let Some(rev) = &live_review {
        s.reviewed_id = rev.get("planId").and_then(Value::as_str).map(str::to_owned);
        s.reviewed_revision = rev.get("revision").and_then(Value::as_u64);
        s.reviewed_digest = rev.get("planDigest").and_then(Value::as_str).map(str::to_owned);
    } else {
        s.reviewed_id = None;
        s.reviewed_revision = None;
        s.reviewed_digest = None;
    }
    s.proposal = live_proposal;
    s.review = live_review;
    s.execution = live_execution;
    s.rendered_samples = rendered_samples;
    s.critique = live_critique;
}

fn render_agy_brain_contents(
    ui: &mut egui::Ui,
    s: &mut ChatState,
    pal: InfernoGlassPalette,
    trigger_agy_direct: &mut bool,
    trigger_undo: &mut bool,
    pending_arabic_feedback: &mut Option<ArabicMotionFeedback>,
) {
    // 1. Execution Scope Toggle + 5 Fleet Node Pills + Collapsible 4-Device IP Config & Live Ping
    Frame::new()
        .fill(pal.bg_card)
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(
                    pal.accent_gold,
                    RichText::new("Execution Scope:").small().strong(),
                );
                let is_device = s.agy_execution_scope == AntigravityExecutionScope::DeviceFleet;
                if ui
                    .selectable_label(
                        is_device,
                        RichText::new("🖥️ Device Agent (Fleet IPs)").small().strong(),
                    )
                    .clicked()
                {
                    s.agy_execution_scope = AntigravityExecutionScope::DeviceFleet;
                }
                let is_remote =
                    s.agy_execution_scope == AntigravityExecutionScope::AccountRemoteAgent;
                if ui
                    .selectable_label(
                        is_remote,
                        RichText::new("🌐 Remote Agent (Account-Bound)")
                            .small()
                            .strong(),
                    )
                    .clicked()
                {
                    s.agy_execution_scope = AntigravityExecutionScope::AccountRemoteAgent;
                }
            });

            if s.agy_execution_scope == AntigravityExecutionScope::AccountRemoteAgent {
                ui.colored_label(
                    pal.accent_emerald,
                    RichText::new(format!(
                        "🌐 Antigravity Remote Agent · GemiAccess -> {AGY_REMOTE_AGENT_RELAY_URL} · Zero IP Config"
                    ))
                    .small()
                    .strong(),
                );
            } else {
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(
                        pal.accent_gold,
                        RichText::new("AGY Fleet Node:").small().strong(),
                    );
                    for node in AntigravityFleetNodeId::ALL {
                        let selected = s.agy_node == node;
                        if ui
                            .selectable_label(
                                selected,
                                RichText::new(node.pill_label()).small().strong(),
                            )
                            .clicked()
                        {
                            s.agy_node = node;
                            let custom = s.agy_fleet_ips.endpoint_for(node).to_string();
                            s.agy_fleet_status =
                                probe_fleet_node_live_status(node, Some(&custom)).ok();
                        }
                    }
                });

                if let Some(st) = &s.agy_fleet_status {
                    ui.horizontal_wrapped(|ui| {
                        let col = if st.reachable {
                            pal.accent_emerald
                        } else {
                            pal.accent_gold
                        };
                        ui.colored_label(col, RichText::new(&st.status_badge).small().strong());
                        ui.colored_label(pal.text_muted, RichText::new(&st.endpoint).small());
                    });
                }

                egui::CollapsingHeader::new(
                    RichText::new("Configure 4-Device IPs & Live Ping")
                        .small()
                        .color(pal.text_secondary),
                )
                .default_open(false)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("💻 Windows:").small());
                        ui.text_edit_singleline(&mut s.agy_fleet_ips.win_local);
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🍎 Mac:").small());
                        ui.text_edit_singleline(&mut s.agy_fleet_ips.mac_local);
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("☁️ VPS 01:").small());
                        ui.text_edit_singleline(&mut s.agy_fleet_ips.vps_01);
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🚀 VPS 02:").small());
                        ui.text_edit_singleline(&mut s.agy_fleet_ips.vps_02);
                    });
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .button(RichText::new("Live Ping Active Node").small().strong())
                            .clicked()
                        {
                            let ep = s.agy_fleet_ips.endpoint_for(s.agy_node).to_string();
                            match probe_fleet_node_live_status(s.agy_node, Some(&ep)) {
                                Ok(st) => {
                                    s.agy_fleet_status = Some(st);
                                    s.error = None;
                                }
                                Err(e) => s.error = Some(e),
                            }
                        }
                        ui.colored_label(
                            pal.text_muted,
                            RichText::new(format!(
                                "Studio Owner Control Hub: {STUDIO_OWNER_CONTROL_HUB_URL}"
                            ))
                            .small(),
                        );
                    });
                });
            }
        });

    ui.add_space(4.0);

    // 2. Selectors for 5 AfterTron Presets, 5 Models, and 4 Effort Tiers
    Frame::new()
        .fill(pal.bg_card)
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.colored_label(
                pal.accent_gold,
                RichText::new("Vibe Preset (5 Motion & VFX Choreography Styles):")
                    .small()
                    .strong(),
            );
            ui.horizontal_wrapped(|ui| {
                for preset in AntigravityVibePreset::ALL {
                    let sel = s.agy_preset == preset;
                    if ui
                        .selectable_label(sel, RichText::new(preset.label()).small())
                        .clicked()
                    {
                        s.agy_preset = preset;
                        s.prompt = preset.default_prompt().to_string();
                    }
                }
            });

            ui.add_space(3.0);
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(
                    pal.text_secondary,
                    RichText::new("Smart Mind Model (14 Brains):").small().strong(),
                );
                egui::ComboBox::from_id_salt("aftertron.agy.smart_mind_14")
                    .selected_text(s.agy_model.label())
                    .show_ui(ui, |ui| {
                        for model in AntigravityModelId::ALL {
                            ui.selectable_value(&mut s.agy_model, model, model.label());
                        }
                    });
                if ui
                    .button(RichText::new("Sync :18220").small())
                    .on_hover_text("Sync live /v1/agy/smart-mind + /v1/agy/health")
                    .clicked()
                {
                    let sync = sync_smart_mind_catalog_from_daemon(None);
                    s.agy_daemon_sync_badge = format!(
                        "{} · {} models · {}",
                        if sync.daemon_online { "● :18220 Online" } else { "✦ Standby" },
                        sync.models.len(),
                        sync.health_status
                    );
                }
                ui.colored_label(pal.text_muted, RichText::new(&s.agy_daemon_sync_badge).small());
            });

            ui.add_space(3.0);
            ui.colored_label(
                pal.text_secondary,
                RichText::new("Effort Tier (5 Tiers · XHigh = 24K Tokens / 4 Passes):")
                    .small()
                    .strong(),
            );
            ui.horizontal_wrapped(|ui| {
                for effort in AntigravityEffortLevel::ALL {
                    let sel = s.agy_effort == effort;
                    if ui
                        .selectable_label(sel, RichText::new(effort.label()).small())
                        .clicked()
                    {
                        s.agy_effort = effort;
                    }
                }
            });

            ui.add_space(4.0);
            ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                ui.colored_label(
                    pal.accent_gold,
                    RichText::new("تحسينات سريعة للموشن والـ VFX (7 تعديلات بضغطة واحدة):")
                        .small()
                        .strong(),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                ui.horizontal_wrapped(|ui| {
                    for fb in ArabicMotionFeedback::ALL {
                        if ui
                            .button(RichText::new(fb.arabic_label()).small())
                            .on_hover_text(fb.english_summary())
                            .clicked()
                        {
                            *pending_arabic_feedback = Some(fb);
                        }
                    }
                });
            });
        });

    if let Some(err) = &s.error {
        ui.add_space(4.0);
        Frame::new()
            .fill(pal.bg_card_selected)
            .stroke(Stroke::new(1.0, pal.accent_crimson))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(Margin::symmetric(8, 5))
            .show(ui, |ui| {
                ui.colored_label(pal.accent_crimson, RichText::new(err).small().strong());
            });
    }

    ui.add_space(4.0);

    // 3. Creator Stream Badges & Closed-Loop Critique Receipt
    ScrollArea::vertical()
        .max_height(225.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if let Some(outcome) = &s.agy_outcome {
                for ev in &outcome.receipt.stream_events {
                    let badge_col = match ev.kind.as_str() {
                        "thinking" => pal.accent_gold,
                        "tool_badge" => pal.accent_crimson,
                        _ => pal.accent_emerald,
                    };
                    Frame::new()
                        .fill(pal.bg_card)
                        .stroke(Stroke::new(1.0, pal.border))
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(Margin::symmetric(8, 6))
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.colored_label(
                                    badge_col,
                                    RichText::new(format!("[{}] {}", ev.kind, ev.badge))
                                        .small()
                                        .strong(),
                                );
                            });
                            draw_bidi_paragraph(ui, &ev.message, pal.text_primary, false);
                        });
                    ui.add_space(4.0);
                }

                let rep = &outcome.receipt.critique_report;
                Frame::new()
                    .fill(pal.bg_card_selected)
                    .stroke(Stroke::new(1.0, pal.accent_emerald))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.colored_label(
                            pal.accent_emerald,
                            RichText::new(format!(
                                "✓ Closed-Loop Critique Receipt · {} steps · {} pass(es)",
                                outcome.receipt.steps_executed, rep.passes_run
                            ))
                            .small()
                            .strong(),
                        );
                        ui.colored_label(
                            pal.text_secondary,
                            RichText::new(format!(
                                "Timebase: {} ticks/sec · Easy-Ease: {} · Glow Hierarchy: {} · WCAG Contrast: {:.2}:1",
                                rep.ticks_per_second,
                                if rep.easy_ease_velocity_continuous { "OK" } else { "Check" },
                                if rep.optical_glow_hierarchy_valid { "OK" } else { "Check" },
                                rep.min_wcag_contrast_ratio
                            ))
                            .small(),
                        );
                        ui.colored_label(
                            pal.text_muted,
                            RichText::new(format!("Receipt Digest: {}", outcome.receipt.sha256_digest))
                                .small(),
                        );
                    });
            } else {
                Frame::new()
                    .fill(pal.bg_card)
                    .stroke(Stroke::new(1.0, pal.border))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        draw_bidi_paragraph(
                            ui,
                            "✦ AGY AI (Host-Inversion MCP Client) — Select a preset, model, effort tier, and fleet node, then click Direct with AGY Brain ✦.",
                            pal.text_secondary,
                            false,
                        );
                    });
            }
        });

    ui.add_space(4.0);
    ui.add(
        egui::TextEdit::multiline(&mut s.prompt)
            .hint_text("Describe your motion & VFX direction for AGY Brain…")
            .desired_rows(2),
    );
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                !s.working && !s.prompt.trim().is_empty(),
                egui::Button::new(
                    RichText::new("Direct with AGY Brain ✦")
                        .color(pal.accent_gold)
                        .strong(),
                ),
            )
            .clicked()
        {
            *trigger_agy_direct = true;
        }
        if s.agy_outcome.is_some()
            && ui
                .button(RichText::new("Undo AGY Choreography ↶").small())
                .clicked()
        {
            *trigger_undo = true;
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn render_chat_contents(
    ui: &mut egui::Ui,
    s: &mut ChatState,
    pal: InfernoGlassPalette,
    real_context_available: bool,
    task: &mut Option<&'static str>,
    motion_prompt: &mut Option<String>,
    switch_direction: &mut Option<usize>,
    native_action: &mut Option<bool>,
    trigger_critique: &mut bool,
    apply_refinement: &mut Option<Value>,
    trigger_undo: &mut bool,
    trigger_agy_direct: &mut bool,
    pending_arabic_feedback: &mut Option<ArabicMotionFeedback>,
) {
    // Top Header Bar
    ui.horizontal(|ui| {
        ui.colored_label(
            pal.accent_crimson,
            RichText::new("◆ TRON").strong().size(15.0),
        );
        ui.colored_label(
            pal.text_secondary,
            RichText::new("AfterTron · المخرج الإبداعي للموشن والـVFX").strong(),
        );
        if let Some(mode) = &s.capture_mode {
            let badge_col = if mode == "LIVE_RUNTIME" {
                pal.accent_emerald
            } else {
                pal.accent_gold
            };
            ui.colored_label(badge_col, RichText::new(format!("[{mode}]")).small().strong());
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .button(RichText::new("—").color(pal.text_primary))
                .on_hover_text("تصغير إلى المشغّل (Minimize)")
                .clicked()
            {
                s.open = false;
                *task = Some("minimize");
            }
            let mode_label = if s.docked { "Float ⧉" } else { "Dock ▥" };
            if ui
                .button(RichText::new(mode_label).color(pal.text_secondary).small())
                .on_hover_text("تبديل الوضع العائم / المثبّت")
                .clicked()
            {
                s.docked = !s.docked;
            }
        });
    });

    ui.add_space(4.0);

    // Unified 3-Brain Switcher (⚡ TRON AI · ✦ AGY AI · ☁️ GemiAI) + 20-Preset GemiTokens Switcher
    ui.horizontal_wrapped(|ui| {
        for mode in DualBrainMode::ALL {
            let selected = s.brain_mode == mode;
            if ui
                .selectable_label(selected, RichText::new(mode.label()).strong())
                .clicked()
            {
                s.brain_mode = mode;
                if mode == DualBrainMode::GemiAi {
                    s.agy_execution_scope = AntigravityExecutionScope::AccountRemoteAgent;
                } else if mode == DualBrainMode::AgyAi && s.agy_fleet_status.is_none() {
                    let ep = s.agy_fleet_ips.endpoint_for(s.agy_node).to_string();
                    s.agy_fleet_status = probe_fleet_node_live_status(s.agy_node, Some(&ep)).ok();
                }
            }
        }
        ui.separator();
        let prev_preset = s.gemi_preset.clone();
        egui::ComboBox::from_id_salt("aftertron.gemi_tokens_20")
            .selected_text(format!("🎨 {}", s.gemi_preset))
            .show_ui(ui, |ui| {
                for pid in crate::gemi_tokens::PRESET_IDS {
                    ui.selectable_value(&mut s.gemi_preset, pid.to_string(), pid);
                }
            });
        if s.gemi_preset != prev_preset {
            let is_dark = ui.visuals().dark_mode;
            if let Some(tokens) = crate::theme::Tokens::for_gemi_preset(&s.gemi_preset, is_dark) {
                crate::theme::apply_visuals(ui.ctx(), &tokens);
            }
        }
    });

    ui.add_space(4.0);

    if matches!(s.brain_mode, DualBrainMode::AgyAi | DualBrainMode::GemiAi) {
        render_agy_brain_contents(
            ui,
            s,
            pal,
            trigger_agy_direct,
            trigger_undo,
            pending_arabic_feedback,
        );
        return;
    }

    // Runtime & GPU Telemetry Strip
    Frame::new()
        .fill(pal.bg_card)
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let status_color = if s.transport_connected && s.power_on {
                    pal.accent_emerald
                } else if s.transport_connected {
                    pal.accent_gold
                } else {
                    pal.text_muted
                };
                let status_label = if s.transport_connected && s.power_on {
                    "● متّصل وجاهز (READY)"
                } else if s.transport_connected {
                    "◐ متّصل (Standby)"
                } else {
                    "○ غير متّصل (Offline)"
                };
                ui.colored_label(status_color, RichText::new(status_label).strong().small());
                ui.separator();
                ui.colored_label(pal.text_secondary, RichText::new(&s.model).small());
                ui.separator();
                ui.colored_label(
                    if s.thinking && s.warmed {
                        pal.accent_emerald
                    } else {
                        pal.text_muted
                    },
                    RichText::new(if s.thinking && s.warmed {
                        "Thinking ✓"
                    } else {
                        "Thinking —"
                    })
                    .small(),
                );
                ui.separator();
                ui.colored_label(
                    pal.text_muted,
                    RichText::new(format!("Leases: {}", s.leases)).small(),
                );
                if let (Some(used), Some(total)) = (s.vram_used_mb, s.vram_total_mb) {
                    ui.separator();
                    ui.colored_label(
                        pal.accent_gold,
                        RichText::new(format!("VRAM: {used}/{total} MB")).small(),
                    );
                }
            });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    if ui
                        .add_enabled(
                            !s.working && !s.power_on,
                            egui::Button::new(RichText::new("تشغيل ON").strong()),
                        )
                        .clicked()
                    {
                        *task = Some("on");
                    }
                    if ui
                        .add_enabled(
                            s.power_on,
                            egui::Button::new(RichText::new("إيقاف OFF")),
                        )
                        .clicked()
                    {
                        *task = Some("off");
                    }
                    if ui
                        .add_enabled(
                            s.working,
                            egui::Button::new(
                                RichText::new("إلغاء Cancel").color(pal.accent_crimson),
                            ),
                        )
                        .clicked()
                    {
                        *task = Some("cancel");
                    }
                }
                #[cfg(target_arch = "wasm32")]
                ui.add_enabled(false, egui::Button::new("Local AI desktop only"));

                if s.working {
                    ui.spinner();
                    ui.colored_label(
                        pal.accent_gold,
                        RichText::new(if s.task_label.is_empty() {
                            "جاري المعالجة..."
                        } else {
                            &s.task_label
                        })
                        .small(),
                    );
                }
            });
        });

    if let Some(err) = &s.error {
        ui.add_space(4.0);
        Frame::new()
            .fill(pal.bg_card_selected)
            .stroke(Stroke::new(1.0, pal.accent_crimson))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(Margin::symmetric(8, 5))
            .show(ui, |ui| {
                ui.colored_label(pal.accent_crimson, RichText::new(err).small().strong());
            });
    }

    ui.add_space(6.0);

    // Main Scrollable Creative Workspace
    ScrollArea::vertical()
        .max_height(355.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Welcome / Creative Persona Banner
            Frame::new()
                .fill(pal.bg_card)
                .stroke(Stroke::new(1.0, pal.border))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    draw_bidi_paragraph(
                        ui,
                        "إخراج موشن جرافيكس احترافي — مش قوالب جاهزة.",
                        pal.accent_gold,
                        true,
                    );
                    draw_bidi_paragraph(
                        ui,
                        "بنقرأ أبعاد الكومبوزيشن، الفريم ريت، وشجرة الطبقات، ونبني اتجاهين إبداعيين للمقارنة قبل تنفيذ أي Keyframe أو Effect.",
                        pal.text_secondary,
                        false,
                    );
                });

            // Conversation History
            if !s.history.is_empty() {
                ui.add_space(6.0);
                for turn in s.history.iter().rev().take(6).rev() {
                    let is_user = turn.role == "user";
                    let card_fill = if is_user {
                        pal.bg_card_selected
                    } else {
                        pal.bg_card
                    };
                    Frame::new()
                        .fill(card_fill)
                        .stroke(Stroke::new(1.0, pal.border))
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let role_title = if is_user {
                                    "أنت (Creative Brief)"
                                } else {
                                    "TRON (المخرج الإبداعي)"
                                };
                                ui.colored_label(
                                    if is_user {
                                        pal.accent_gold
                                    } else {
                                        pal.accent_crimson
                                    },
                                    RichText::new(role_title).small().strong(),
                                );
                                if let Some(b) = &turn.badge {
                                    ui.colored_label(pal.text_muted, RichText::new(b).small());
                                }
                            });
                            draw_bidi_paragraph(ui, &turn.text, pal.text_primary, false);
                        });
                    ui.add_space(4.0);
                }
            }

            // Two Creative Direction Comparison Cards
            if let Some(last) = s.proposal.clone() {
                let proposal = last.get("result").unwrap_or(&last);
                if let Some(directions) = proposal
                    .get("exploredDirections")
                    .and_then(Value::as_array)
                    .filter(|d| !d.is_empty())
                {
                    ui.add_space(4.0);
                    draw_bidi_paragraph(
                        ui,
                        "مقارنة الاتجاهات الإبداعية المقترحة (اختر الاتجاه الأنسب):",
                        pal.accent_gold,
                        true,
                    );
                    for (idx, dir) in directions.iter().take(2).enumerate() {
                        let is_selected = s.selected_direction == idx;
                        let title = dir
                            .get("label")
                            .or_else(|| dir.get("title"))
                            .and_then(Value::as_str)
                            .unwrap_or(if idx == 0 {
                                "Direction A"
                            } else {
                                "Direction B"
                            });
                        let summary = dir
                            .get("summary")
                            .or_else(|| dir.get("rationale"))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        let step_count = dir
                            .get("plan")
                            .and_then(|p| p.get("steps"))
                            .and_then(Value::as_array)
                            .map_or(0, Vec::len);

                        Frame::new()
                            .fill(if is_selected {
                                pal.bg_card_selected
                            } else {
                                pal.bg_card
                            })
                            .stroke(Stroke::new(
                                if is_selected { 1.5 } else { 1.0 },
                                if is_selected {
                                    pal.border_active
                                } else {
                                    pal.border
                                },
                            ))
                            .corner_radius(CornerRadius::same(6))
                            .inner_margin(Margin::symmetric(10, 8))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let badge = if is_selected {
                                        "★ الاتجاه المختار"
                                    } else {
                                        "اتجاه بديل"
                                    };
                                    ui.colored_label(
                                        if is_selected {
                                            pal.accent_crimson
                                        } else {
                                            pal.text_muted
                                        },
                                        RichText::new(badge).small().strong(),
                                    );
                                    ui.colored_label(
                                        pal.text_muted,
                                        RichText::new(format!("({step_count} خطوات أصلية)"))
                                            .small(),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let btn_text = if is_selected {
                                            "مُحدّد ✓"
                                        } else {
                                            "اختيار هذا الاتجاه"
                                        };
                                        if ui
                                            .add_enabled(
                                                !is_selected && !s.working,
                                                egui::Button::new(RichText::new(btn_text).small()),
                                            )
                                            .clicked()
                                        {
                                            *switch_direction = Some(idx);
                                        }
                                    });
                                });
                                draw_bidi_paragraph(ui, title, pal.text_primary, true);
                                if !summary.is_empty() {
                                    draw_bidi_paragraph(ui, summary, pal.text_secondary, false);
                                }
                            });
                        ui.add_space(4.0);
                    }
                }

                // Proposed Native Operations & Approval Gate
                if let Some(steps) = proposal
                    .get("plan")
                    .and_then(|p| p.get("steps"))
                    .and_then(Value::as_array)
                {
                    Frame::new()
                        .fill(pal.bg_card)
                        .stroke(Stroke::new(1.0, pal.border))
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.colored_label(
                                pal.text_primary,
                                RichText::new(format!(
                                    "العمليات الأصلية المقترحة ({} خطوات — لم تُنفّذ بعد):",
                                    steps.len()
                                ))
                                .strong(),
                            );
                            for step in steps.iter().take(16) {
                                ui.colored_label(
                                    pal.text_secondary,
                                    RichText::new(format!(
                                        "• {} → {}",
                                        step.get("id").and_then(Value::as_str).unwrap_or("?"),
                                        step.get("tool").and_then(Value::as_str).unwrap_or("?")
                                    ))
                                    .small(),
                                );
                            }
                            if s.power_on && !s.working {
                                ui.add_space(4.0);
                                ui.horizontal_wrapped(|ui| {
                                    if ui
                                        .button(
                                            RichText::new("1 · فحص الخطة (Native Preflight)")
                                                .strong(),
                                        )
                                        .clicked()
                                    {
                                        *native_action = Some(false);
                                    }
                                    if let Some(rev_info) = &s.review {
                                        let digest = rev_info
                                            .get("planDigest")
                                            .and_then(Value::as_str)
                                            .unwrap_or("-");
                                        ui.colored_label(
                                            pal.accent_emerald,
                                            RichText::new(format!("✓ Preflight OK ({digest})"))
                                                .small()
                                                .strong(),
                                        );
                                        if ui
                                            .button(
                                                RichText::new(
                                                    "2 · اعتماد وتنفيذ الحركة على الـComposition",
                                                )
                                                .color(pal.accent_gold)
                                                .strong(),
                                            )
                                            .clicked()
                                        {
                                            *native_action = Some(true);
                                        }
                                    }
                                });
                            }
                        });
                }
            }

            // Execution Receipt + 5-Frame Visual Critique + Undo/Refinement
            if let Some(result) = &s.execution {
                ui.add_space(6.0);
                Frame::new()
                    .fill(pal.bg_card)
                    .stroke(Stroke::new(1.0, pal.accent_emerald))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        let status = result["native"]["status"]
                            .as_str()
                            .unwrap_or("EXECUTED_NATIVE");
                        let after_rev = result["afterRevision"].as_u64().unwrap_or(0);
                        ui.colored_label(
                            pal.accent_emerald,
                            RichText::new(format!(
                                "✓ تم التنفيذ الأصلي ({status} · rev {after_rev})"
                            ))
                            .strong(),
                        );
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .add_enabled(
                                    !s.working && s.power_on,
                                    egui::Button::new(
                                        RichText::new("3 · معاينة ونقد 5 كادرات حقيقية").strong(),
                                    ),
                                )
                                .clicked()
                            {
                                *trigger_critique = true;
                            }
                            if ui
                                .add_enabled(
                                    !s.working,
                                    egui::Button::new(RichText::new(
                                        "تراجع بخطوة واحدة (Undo Plan)",
                                    )),
                                )
                                .clicked()
                            {
                                *trigger_undo = true;
                            }
                        });
                    });
            }

            if let Some(crit_raw) = &s.critique {
                let crit = crit_raw.get("critique").unwrap_or(crit_raw);
                ui.add_space(6.0);
                Frame::new()
                    .fill(pal.bg_card_selected)
                    .stroke(Stroke::new(1.0, pal.accent_gold))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.colored_label(
                                pal.accent_gold,
                                RichText::new("النقد البصري لـ 5 كادرات مرسومة (0% · 25% · 50% · 75% · 100%)")
                                    .strong(),
                            );
                            let vis_ok = crit
                                .get("visionInspected")
                                .and_then(Value::as_bool)
                                .unwrap_or(false);
                            if vis_ok {
                                ui.colored_label(
                                    pal.accent_emerald,
                                    RichText::new("● Vision Multimodal Verified").small().strong(),
                                );
                            }
                        });
                        if let Some(text) = crit.get("critiqueText").and_then(Value::as_str) {
                            draw_bidi_paragraph(ui, text, pal.text_primary, false);
                        }
                        if !s.rendered_samples.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                for sample in &s.rendered_samples {
                                    let lbl = sample["label"].as_str().unwrap_or("?");
                                    let luma = sample["meanLuma"].as_f64().unwrap_or(0.0);
                                    let alpha = sample["nonZeroAlphaRatio"].as_f64().unwrap_or(0.0);
                                    ui.colored_label(
                                        pal.text_muted,
                                        RichText::new(format!(
                                            "[{lbl}: Y={luma:.1} α={:.0}%]",
                                            alpha * 100.0
                                        ))
                                        .small(),
                                    );
                                }
                            });
                        }
                        if let Some(ref_plan) = crit.get("refinementPlan").filter(|v| v.is_object())
                            && ui
                                .add_enabled(
                                    !s.working && s.power_on,
                                    egui::Button::new(
                                        RichText::new("تطبيق خطة التحسين البصري (Load Refinement)")
                                            .strong(),
                                    ),
                                )
                                .clicked()
                        {
                            *apply_refinement = Some(ref_plan.clone());
                        }
                    });
            }
        });

    ui.add_space(6.0);
    let enabled = s.power_on && !s.working && real_context_available;
    ui.add_enabled(
        enabled,
        egui::TextEdit::multiline(&mut s.prompt)
            .hint_text("اكتب تفاصيل البريف الإبداعي، التايبوجرافي، الإيقاع، أو الملاحظات البصرية...")
            .desired_rows(2),
    );
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                enabled && !s.prompt.trim().is_empty(),
                egui::Button::new(
                    RichText::new("فكّر واقترح اتجاهين (Generate Motion Plan)").strong(),
                ),
            )
            .clicked()
        {
            *motion_prompt = Some(s.prompt.clone());
        }
        if !real_context_available {
            ui.colored_label(
                pal.text_muted,
                RichText::new("افتح Composition لتفعيل التخطيط").small(),
            );
        }
    });
}

/// Native app UI state is read at each frame, not captured before model
/// inference. The review and approval steps use the same current Session.
pub fn show(ctx: &Context, session: &mut effectcraft_engine::Session) {
    let st = shared(ctx);
    let dark_mode = ctx.global_style().visuals.dark_mode;
    let pal = palette_for(dark_mode);
    let (is_open, is_docked, _is_working, is_power_on) = st
        .lock()
        .map(|s| (s.open, s.docked, s.working, s.power_on))
        .unwrap_or((false, false, false, false));

    if !is_open {
        egui::Area::new(Id::new("aftertron.tron.launcher"))
            .order(Order::Foreground)
            .anchor(Align2::RIGHT_BOTTOM, vec2(-22.0, -26.0))
            .show(ctx, |ui| {
                Frame::new()
                    .fill(pal.bg_primary)
                    .stroke(Stroke::new(1.5, pal.border_active))
                    .corner_radius(CornerRadius::same(20))
                    .inner_margin(Margin::symmetric(14, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let dot_color = if is_power_on {
                                pal.accent_emerald
                            } else {
                                pal.accent_crimson
                            };
                            ui.colored_label(dot_color, "◆");
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("TRON · المخرج الإبداعي")
                                            .color(pal.text_primary)
                                            .strong(),
                                    )
                                    .frame(false),
                                )
                                .clicked()
                                && let Ok(mut s) = st.lock()
                            {
                                s.open = true;
                                s.last_status = Instant::now() - Duration::from_secs(20);
                            }
                        });
                    });
            });
        return;
    }

    let poll = st
        .lock()
        .map(|s| !s.working && s.last_status.elapsed() > Duration::from_secs(12))
        .unwrap_or(false);
    #[cfg(not(target_arch = "wasm32"))]
    if poll {
        action(
            st.clone(),
            ctx.clone(),
            if is_power_on { "heartbeat" } else { "status" },
        );
    }
    #[cfg(target_arch = "wasm32")]
    let _ = (poll, is_working);

    let real_context = crate::tron_actions::live_context(session);
    let mut task = None;
    let mut motion_prompt = None;
    let mut switch_direction = None;
    let mut native_action: Option<bool> = None;
    let mut trigger_critique = false;
    let mut apply_refinement: Option<Value> = None;
    let mut trigger_undo = false;
    let mut trigger_agy_direct = false;
    let mut pending_arabic_feedback: Option<ArabicMotionFeedback> = None;

    let bounds = ctx.content_rect();
    let win_frame = Frame::window(&ctx.global_style())
        .fill(pal.bg_primary)
        .stroke(Stroke::new(1.2, pal.border))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(12));

    let mut win = egui::Window::new("TRON · Motion Creative Director")
        .id(Id::new("aftertron.tron.window"))
        .order(Order::Foreground)
        .title_bar(false)
        .frame(win_frame)
        .min_size(vec2(380.0, 340.0))
        .collapsible(false);

    if is_docked {
        win = win
            .fixed_pos(egui::pos2(
                (bounds.right() - 462.0).max(bounds.left() + 8.0),
                bounds.top() + 66.0,
            ))
            .fixed_size(vec2(450.0, (bounds.height() - 96.0).max(360.0)))
            .resizable(false);
    } else {
        win = win
            .default_pos(egui::pos2(
                (bounds.right() - 472.0).max(bounds.left() + 8.0),
                bounds.top() + 66.0,
            ))
            .default_size(vec2(456.0, 580.0))
            .resizable(true);
    }

    win.show(ctx, |ui| {
        let Ok(mut s) = st.lock() else { return };
        render_chat_contents(
            ui,
            &mut s,
            pal,
            real_context.is_some(),
            &mut task,
            &mut motion_prompt,
            &mut switch_direction,
            &mut native_action,
            &mut trigger_critique,
            &mut apply_refinement,
            &mut trigger_undo,
            &mut trigger_agy_direct,
            &mut pending_arabic_feedback,
        );
    });

    if let Some(fb) = pending_arabic_feedback
        && let Ok(mut s) = st.lock()
    {
        match apply_arabic_motion_feedback_session(session, fb) {
            Ok(summary) => {
                s.history.push(ChatTurn {
                    role: "tron".into(),
                    text: summary,
                    badge: Some("Arabic VFX Refinement · 1-Click Undo".into()),
                });
                s.error = None;
            }
            Err(e) => s.error = Some(e),
        }
    }

    if trigger_agy_direct && let Ok(mut s) = st.lock() {
        let ep = s.agy_fleet_ips.endpoint_for(s.agy_node).to_string();
        let req = AntigravityDirectRequest {
            prompt: s.prompt.clone(),
            preset: s.agy_preset,
            model: s.agy_model,
            effort: s.agy_effort,
            auth_mode: AntigravityAuthMode::AgyMcpHost,
            execution_scope: s.agy_execution_scope,
            node: s.agy_node,
            custom_node_endpoint: Some(ep),
        };
        match direct_with_antigravity_brain_session(session, &req) {
            Ok(outcome) => {
                s.agy_fleet_status = Some(outcome.fleet_status.clone());
                s.agy_outcome = Some(outcome);
                s.error = None;
            }
            Err(e) => {
                s.error = Some(e);
            }
        }
    }

    if let Some(dir_idx) = switch_direction
        && let Ok(mut s) = st.lock()
        && let Some(prop) = s.proposal.clone()
    {
        match crate::tron_actions::select_direction(&prop, dir_idx) {
            Ok(updated) => {
                s.selected_direction = dir_idx;
                s.proposal = Some(updated);
                s.review = None;
                s.reviewed_id = None;
                s.reviewed_revision = None;
                s.reviewed_digest = None;
                s.error = None;
            }
            Err(e) => s.error = Some(e),
        }
    }

    if let Some(ref_plan) = apply_refinement
        && let Ok(mut s) = st.lock()
    {
        s.proposal = Some(json!({
            "ok": true,
            "result": {
                "ok": true,
                "userExplanation": "خطة تحسين إبداعي مبنية على فحص الـ 5 كادرات الحقيقية.",
                "plan": ref_plan
            }
        }));
        s.review = None;
        s.reviewed_id = None;
        s.reviewed_revision = None;
        s.reviewed_digest = None;
    }

    if trigger_undo {
        let _ = session.undo();
        if let Ok(mut s) = st.lock() {
            s.execution = None;
            s.critique = None;
            s.agy_outcome = None;
            s.rendered_samples.clear();
            s.history.push(ChatTurn {
                role: "tron".into(),
                text: "تم التراجع عن خطة الحركة بالكامل في خطوة Undo واحدة.".into(),
                badge: Some("Undo".into()),
            });
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    if let Some(task) = task {
        action(st.clone(), ctx.clone(), task);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(prompt) = motion_prompt
        && let Some(context) = real_context.clone()
    {
        generate(st.clone(), ctx.clone(), prompt, context);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if trigger_critique
        && let Some(context) = crate::tron_actions::live_context(session)
    {
        match crate::tron_actions::sample_rendered_frames(session) {
            Ok(samples) => {
                let prompt = st
                    .lock()
                    .map(|s| s.prompt.clone())
                    .unwrap_or_default();
                run_visual_critique(st.clone(), ctx.clone(), context, samples, prompt);
            }
            Err(e) => {
                if let Ok(mut s) = st.lock() {
                    s.error = Some(e);
                }
            }
        }
    }

    if let Some(approved) = native_action
        && let Ok(mut s) = st.lock()
    {
        let result = match s.proposal.clone() {
            Some(plan) if s.power_on && !s.working => {
                if approved {
                    match (
                        s.reviewed_id.clone(),
                        s.reviewed_revision,
                        s.reviewed_digest.clone(),
                    ) {
                        (Some(id), Some(rev), Some(digest)) => {
                            crate::tron_actions::apply(session, &plan, &id, rev, &digest)
                        }
                        _ => Err("AFTERTRON_PLAN_NOT_REVIEWED".into()),
                    }
                } else {
                    crate::tron_actions::review(session, &plan)
                }
            }
            _ => Err("AFTERTRON_MODEL_OFF_OR_NO_PLAN".into()),
        };
        match result {
            Ok(v) if approved => {
                s.execution = Some(v);
                s.review = None;
                s.reviewed_id = None;
                s.reviewed_revision = None;
                s.reviewed_digest = None;
                s.error = None;
            }
            Ok(v) => {
                s.reviewed_id = v.get("planId").and_then(Value::as_str).map(str::to_owned);
                s.reviewed_revision = v.get("revision").and_then(Value::as_u64);
                s.reviewed_digest = v
                    .get("planDigest")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                s.review = Some(v);
                s.error = None;
            }
            Err(e) => {
                s.error = Some(e);
                s.review = None;
                s.reviewed_id = None;
                s.reviewed_revision = None;
                s.reviewed_digest = None;
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = (task, motion_prompt, trigger_critique);
    ctx.request_repaint_after(Duration::from_secs(2));
}

#[cfg(not(target_arch = "wasm32"))]
pub fn release_on_exit() {
    let _ = effectcraft_automation::tron_local::cancel_job();
    let _ = effectcraft_automation::tron_local::release();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_is_never_preloaded_on_window_construction() {
        let s = ChatState::default();
        assert!(!s.open);
        assert!(!s.power_on);
        assert!(!s.transport_connected);
        assert_eq!(s.model_state, "NOT_LOADED");
    }

    #[test]
    fn unified_brain_3_modes_14_models_5_efforts_xhigh_and_20_gemi_presets() {
        assert_eq!(DualBrainMode::ALL.len(), 3);
        assert_eq!(DualBrainMode::TronAi.label(), "⚡ TRON AI");
        assert_eq!(DualBrainMode::AgyAi.label(), "✦ AGY AI");
        assert_eq!(DualBrainMode::GemiAi.label(), "☁️ GemiAI");
        assert_eq!(AntigravityModelId::ALL.len(), 14);
        assert_eq!(AntigravityEffortLevel::ALL.len(), 5);
        assert_eq!(AntigravityEffortLevel::Xhigh.thinking_budget_tokens(), 24_576);
        assert_eq!(AntigravityEffortLevel::Xhigh.max_critique_passes(), 4);
        assert_eq!(ArabicMotionFeedback::ALL.len(), 7);
        assert_eq!(crate::gemi_tokens::PRESET_IDS.len(), 20);
    }
}

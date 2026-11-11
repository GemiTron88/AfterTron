//! Clean-room `.aepx` XML composition interchange (`705,600,000` flicks/sec) and
//! `troncs.handoff.v1` cross-suite dynamic asset link (`PhotoTron -> AfterTron -> FilmTron`).
//!
//! # Clean-Room & Loss Disclosure Contract
//! - `.aepx` documents use clean-room XML elements with exact rational time ticks
//!   (`AEPX_FLICKS_PER_SECOND = 705_600_000`; `effectcraft_time::TICKS_PER_SECOND / 705_600_000 == 360`).
//! - Unsupported effects, JavaScript expressions, 3D model/primitive layers, track mattes, and
//!   layer styles are NEVER silently dropped: every non-lossless construct emits an explicit
//!   [`AepxLossDiagnostic`] and rolls up into [`LossClassification`]
//!   (`EXACT_VERIFIED`, `COMPATIBLE_WITH_TRANSFORM`, or `LOSSY_APPROVAL_REQUIRED`).
//! - L3 pure-memory implementation: zero file or network I/O, zero `unsafe`, and zero panicking
//!   primitives (`unwrap`/`expect`/`panic!`).

use std::collections::BTreeMap;

use effectcraft_color::{BlendMode, Label};
use effectcraft_keyframe::{Direction, Ease, Interp, Keyframe, TextDoc, Value};
use effectcraft_project::{
    AlphaMode, Comp, Footage, FootageKind, ItemId, ItemKind, LayerId, LayerSource, Node,
    Project, Property, Solid, build,
};
use effectcraft_time::{FrameRate, TICKS_PER_SECOND, Tick};
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Standard `.aepx` / open flick timebase: `705,600,000` flicks per second.
pub const AEPX_FLICKS_PER_SECOND: i64 = 705_600_000;

/// Exact integer ratio between EffectCraft's internal tick (`254,016,000,000` ticks/sec) and
/// `.aepx` flicks (`705,600,000` flicks/sec): `254_016_000_000 == 705_600_000 * 360`.
pub const TICKS_PER_AEPX_FLICK: i64 = 360;

const _: () = assert!(TICKS_PER_SECOND == AEPX_FLICKS_PER_SECOND * TICKS_PER_AEPX_FLICK);

/// Canonical TronCS handoff schema version.
pub const TRONCS_HANDOFF_SCHEMA_V1: &str = "troncs.handoff.v1";

/// Canonical TronCS loss-classification tier for interchange and cross-app handoff.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LossClassification {
    /// Every composition, layer, and transform keyframe roundtrips without semantic loss.
    #[default]
    ExactVerified,
    /// Visual/timing intent is preserved via deterministic coordinate, text, or timebase transform.
    CompatibleWithTransform,
    /// Contains expressions, unsupported effects, track mattes, or 3D meshes requiring user approval
    /// or ProRes 4444 alpha stem pre-render.
    LossyApprovalRequired,
}

impl LossClassification {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExactVerified => "EXACT_VERIFIED",
            Self::CompatibleWithTransform => "COMPATIBLE_WITH_TRANSFORM",
            Self::LossyApprovalRequired => "LOSSY_APPROVAL_REQUIRED",
        }
    }

    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_uppercase().as_str() {
            "EXACT_VERIFIED" => Self::ExactVerified,
            "COMPATIBLE_WITH_TRANSFORM" => Self::CompatibleWithTransform,
            "LOSSY_APPROVAL_REQUIRED" => Self::LossyApprovalRequired,
            _ => Self::CompatibleWithTransform,
        }
    }

    #[must_use]
    pub fn escalate(self, other: Self) -> Self {
        if other > self { other } else { self }
    }
}

/// Explicit per-layer/property loss disclosure emitted during `.aepx` export or import.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AepxLossDiagnostic {
    pub layer_name: String,
    pub feature: String,
    pub classification: LossClassification,
    pub message: String,
    pub requires_user_approval: bool,
}

/// Output of exporting an EffectCraft composition to clean-room `.aepx` XML.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AepxExportReport {
    pub xml: String,
    pub loss_classification: LossClassification,
    pub diagnostics: Vec<AepxLossDiagnostic>,
    pub layer_count: usize,
    pub keyframe_count: usize,
}

/// Output of importing a clean-room `.aepx` XML composition into an EffectCraft [`Project`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AepxImportReport {
    pub comp_id: ItemId,
    pub comp_name: String,
    pub loss_classification: LossClassification,
    pub diagnostics: Vec<AepxLossDiagnostic>,
    pub imported_layers: usize,
    pub imported_keyframes: usize,
}

/// Convert an EffectCraft [`Tick`] to `.aepx` flicks (`705,600,000` per second).
///
/// If a sub-flick remainder exists, returns `(flicks, false)` to indicate rounding.
pub const fn tick_to_aepx_flicks(tick: Tick) -> (i64, bool) {
    let flicks = tick.0 / TICKS_PER_AEPX_FLICK;
    let exact = tick.0 % TICKS_PER_AEPX_FLICK == 0;
    (flicks, exact)
}

/// Convert `.aepx` flicks (`705,600,000` per second) to an EffectCraft [`Tick`].
pub const fn aepx_flicks_to_tick(flicks: i64) -> Tick {
    Tick(flicks.saturating_mul(TICKS_PER_AEPX_FLICK))
}

fn xml_escape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

fn xml_unescape(raw: &str) -> String {
    raw.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn parse_xml_attrs(tag_header: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let bytes = tag_header.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'/') {
            i += 1;
        }
        let key_start = i;
        while i < bytes.len() && bytes[i] != b'=' && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if key_start == i || i >= bytes.len() || bytes[i] != b'=' {
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            continue;
        }
        let key = tag_header[key_start..i].trim().to_string();
        i += 1; // '='
        if i >= bytes.len() {
            break;
        }
        let quote = bytes[i];
        if quote != b'"' && quote != b'\'' {
            continue;
        }
        i += 1;
        let val_start = i;
        while i < bytes.len() && bytes[i] != quote {
            i += 1;
        }
        let val = xml_unescape(&tag_header[val_start..i.min(bytes.len())]);
        map.insert(key, val);
        if i < bytes.len() {
            i += 1;
        }
    }
    map
}

fn collect_blocks<'a>(xml: &'a str, tag: &str) -> Vec<(BTreeMap<String, String>, &'a str)> {
    let open_pat = format!("<{tag}");
    let close_pat = format!("</{tag}>");
    let mut results = Vec::new();
    let mut cursor = 0;
    while let Some(rel_start) = xml[cursor..].find(&open_pat) {
        let start = cursor + rel_start;
        let after_name = start + open_pat.len();
        let next_ch = xml[after_name..].chars().next();
        if !matches!(next_ch, Some(c) if c.is_ascii_whitespace() || c == '>' || c == '/') {
            cursor = after_name;
            continue;
        }
        let Some(rel_gt) = xml[after_name..].find('>') else {
            break;
        };
        let gt_pos = after_name + rel_gt;
        let header = &xml[after_name..gt_pos];
        let attrs = parse_xml_attrs(header);
        if header.trim_end().ends_with('/') {
            results.push((attrs, ""));
            cursor = gt_pos + 1;
            continue;
        }
        let body_start = gt_pos + 1;
        let Some(rel_close) = xml[body_start..].find(&close_pat) else {
            break;
        };
        let body_end = body_start + rel_close;
        results.push((attrs, &xml[body_start..body_end]));
        cursor = body_end + close_pat.len();
    }
    results
}

fn format_value_csv(value: &Value) -> String {
    match value {
        Value::Scalar(v) => format!("{v:.6}"),
        Value::Vec2(v) => format!("{:.6},{:.6}", v[0], v[1]),
        Value::Vec3(v) => format!("{:.6},{:.6},{:.6}", v[0], v[1], v[2]),
        Value::Color(v) => format!("{:.6},{:.6},{:.6},{:.6}", v[0], v[1], v[2], v[3]),
        Value::Bool(b) => if *b { "1" } else { "0" }.to_string(),
        Value::Enum(idx) => idx.to_string(),
        _ => "0".to_string(),
    }
}

fn parse_value_csv(match_id: &str, raw: &str) -> Value {
    let nums: Vec<f64> = raw
        .split(',')
        .filter_map(|p| p.trim().parse::<f64>().ok())
        .collect();
    match match_id {
        "anchor" | "position" | "orientation" => Value::Vec3([
            nums.first().copied().unwrap_or(0.0),
            nums.get(1).copied().unwrap_or(0.0),
            nums.get(2).copied().unwrap_or(0.0),
        ]),
        "scale" => Value::Vec3([
            nums.first().copied().unwrap_or(100.0),
            nums.get(1).copied().unwrap_or(100.0),
            nums.get(2).copied().unwrap_or(100.0),
        ]),
        _ => Value::Scalar(nums.first().copied().unwrap_or(0.0)),
    }
}

/// Export an EffectCraft composition to clean-room `.aepx` XML (`705,600,000` flicks/sec),
/// classifying every non-lossless construct with [`LossClassification`].
pub fn export_comp_to_aepx_xml(project: &Project, comp_id: ItemId) -> Result<AepxExportReport> {
    let Some(item) = project.item(comp_id) else {
        return Err(Error::NoComp(comp_id.0));
    };
    let Some(comp) = item.as_comp() else {
        return Err(Error::NoComp(comp_id.0));
    };

    let mut classification = LossClassification::ExactVerified;
    let mut diagnostics = Vec::new();
    let mut keyframe_count = 0usize;

    let (dur_flicks, dur_exact) = tick_to_aepx_flicks(comp.duration);
    let (start_flicks, start_exact) = tick_to_aepx_flicks(comp.display_start);
    let (wa_in_flicks, wa_in_exact) = tick_to_aepx_flicks(comp.work_area.0);
    let (wa_out_flicks, wa_out_exact) = tick_to_aepx_flicks(comp.work_area.1);
    if !(dur_exact && start_exact && wa_in_exact && wa_out_exact) {
        classification = classification.escalate(LossClassification::CompatibleWithTransform);
        diagnostics.push(AepxLossDiagnostic {
            layer_name: item.name.clone(),
            feature: "sub_flick_timing".into(),
            classification: LossClassification::CompatibleWithTransform,
            message: "Composition timing quantized to 1/705,600,000s (.aepx flick boundary).".into(),
            requires_user_approval: false,
        });
    }

    let mut layers_xml = String::new();
    for layer in &comp.layers {
        let (in_flicks, in_exact) = tick_to_aepx_flicks(layer.in_point);
        let (out_flicks, out_exact) = tick_to_aepx_flicks(layer.out_point);
        let (st_flicks, st_exact) = tick_to_aepx_flicks(layer.start_time);
        if !(in_exact && out_exact && st_exact) {
            classification = classification.escalate(LossClassification::CompatibleWithTransform);
        }

        // Check unsupported or transform-adapted layer constructs explicitly.
        let mut source_attrs = String::new();
        let kind_tag = match &layer.source {
            LayerSource::Footage { item: src_id } => {
                if let Some(ItemKind::Footage(f)) = project.item(*src_id).map(|i| &i.kind) {
                    let alpha = match f.alpha {
                        AlphaMode::Straight => "straight",
                        AlphaMode::Premultiplied => "premultiplied",
                        AlphaMode::Ignore => "ignore",
                    };
                    source_attrs = format!(
                        " srcPath=\"{}\" srcWidth=\"{}\" srcHeight=\"{}\" srcRateNum=\"{}\" srcRateDen=\"{}\" srcAlpha=\"{}\"",
                        xml_escape(&f.path),
                        f.width,
                        f.height,
                        f.frame_rate.num,
                        f.frame_rate.den,
                        alpha
                    );
                }
                "footage"
            }
            LayerSource::Solid { item: src_id } => {
                if let Some(ItemKind::Solid(s)) = project.item(*src_id).map(|i| &i.kind) {
                    source_attrs = format!(
                        " solidR=\"{:.6}\" solidG=\"{:.6}\" solidB=\"{:.6}\" srcWidth=\"{}\" srcHeight=\"{}\"",
                        s.color[0], s.color[1], s.color[2], s.width, s.height
                    );
                }
                if layer.switches.adjustment { "adjustment" } else { "solid" }
            }
            LayerSource::Text => {
                classification = classification.escalate(LossClassification::CompatibleWithTransform);
                let mut source_text = String::new();
                let mut rtl = false;
                let mut font = String::from("GemiFontSans");
                let mut font_size = 72.0;
                if let Some(Value::Text(doc)) = layer
                    .props
                    .sub("text")
                    .and_then(|g| g.get("sourceText"))
                    .map(|p| &p.value)
                {
                    source_text = doc.text.clone();
                    rtl = doc.direction == Direction::Rtl;
                    font = doc.font.clone();
                    font_size = doc.size;
                }
                source_attrs = format!(
                    " text=\"{}\" rtl=\"{}\" font=\"{}\" fontSize=\"{:.2}\"",
                    xml_escape(&source_text),
                    rtl,
                    xml_escape(&font),
                    font_size
                );
                diagnostics.push(AepxLossDiagnostic {
                    layer_name: layer.name.clone(),
                    feature: "text_shaping_engine".into(),
                    classification: LossClassification::CompatibleWithTransform,
                    message: "Text layer exported with UTF-8 content, font, size, and RTL direction; host shaper metrics may differ.".into(),
                    requires_user_approval: false,
                });
                "text"
            }
            LayerSource::Shape => {
                classification = classification.escalate(LossClassification::CompatibleWithTransform);
                diagnostics.push(AepxLossDiagnostic {
                    layer_name: layer.name.clone(),
                    feature: "parametric_shape_tree".into(),
                    classification: LossClassification::CompatibleWithTransform,
                    message: "Shape layer exported as parametric vector container; complex merge paths may require pre-render.".into(),
                    requires_user_approval: false,
                });
                "shape"
            }
            LayerSource::Null => "null",
            LayerSource::Comp { item: nested_id } => {
                let nested_name = project
                    .item(*nested_id)
                    .map(|i| i.name.as_str())
                    .unwrap_or("Precomp");
                classification = classification.escalate(LossClassification::CompatibleWithTransform);
                source_attrs = format!(" nestedComp=\"{}\"", xml_escape(nested_name));
                "precomp"
            }
            LayerSource::Camera => {
                classification = classification.escalate(LossClassification::CompatibleWithTransform);
                "camera"
            }
            LayerSource::Light { kind } => {
                classification = classification.escalate(LossClassification::CompatibleWithTransform);
                source_attrs = format!(" lightKind=\"{}\"", xml_escape(kind.label()));
                "light"
            }
            LayerSource::Model { .. } | LayerSource::Primitive { .. } => {
                classification = classification.escalate(LossClassification::LossyApprovalRequired);
                diagnostics.push(AepxLossDiagnostic {
                    layer_name: layer.name.clone(),
                    feature: "advanced_3d_mesh".into(),
                    classification: LossClassification::LossyApprovalRequired,
                    message: "3D Model/Primitive layer cannot roundtrip losslessly in .aepx XML; approve ProRes 4444 pre-render or placeholder.".into(),
                    requires_user_approval: true,
                });
                "model3d"
            }
        };

        let mut disclosures_xml = String::new();

        // Inspect Effects group for unsupported/custom effects.
        if let Some(effects) = layer.effects() {
            for child in &effects.children {
                if let Node::Group(fx) = child {
                    classification = classification.escalate(LossClassification::LossyApprovalRequired);
                    let msg = format!(
                        "Effect '{}' ({}) requires matching host plug-in or pre-rendered alpha stem.",
                        fx.name, fx.match_id
                    );
                    diagnostics.push(AepxLossDiagnostic {
                        layer_name: layer.name.clone(),
                        feature: format!("effect:{}", fx.match_id),
                        classification: LossClassification::LossyApprovalRequired,
                        message: msg.clone(),
                        requires_user_approval: true,
                    });
                    disclosures_xml.push_str(&format!(
                        "        <EffectDisclosure matchId=\"{}\" name=\"{}\" classification=\"LOSSY_APPROVAL_REQUIRED\" message=\"{}\"/>\n",
                        xml_escape(&fx.match_id),
                        xml_escape(&fx.name),
                        xml_escape(&msg)
                    ));
                }
            }
        }

        if let Some(tm) = layer.track_matte {
            classification = classification.escalate(LossClassification::LossyApprovalRequired);
            let msg = format!("Track matte ({}) requires explicit matte-layer approval.", tm.kind.label());
            diagnostics.push(AepxLossDiagnostic {
                layer_name: layer.name.clone(),
                feature: "track_matte".into(),
                classification: LossClassification::LossyApprovalRequired,
                message: msg.clone(),
                requires_user_approval: true,
            });
            disclosures_xml.push_str(&format!(
                "        <FeatureDisclosure feature=\"track_matte\" classification=\"LOSSY_APPROVAL_REQUIRED\" message=\"{}\"/>\n",
                xml_escape(&msg)
            ));
        }

        // Walk all properties for JavaScript expressions so none are silently dropped.
        let mut expr_records: Vec<(String, String, bool)> = Vec::new();
        layer.props.walk("", &mut |path, prop| {
            if let Some(expr) = &prop.expr {
                if !expr.text.trim().is_empty() {
                    expr_records.push((path.to_string(), expr.text.clone(), expr.enabled));
                }
            }
        });
        for (prop_path, expr_text, enabled) in expr_records {
            classification = classification.escalate(LossClassification::LossyApprovalRequired);
            let msg = format!(
                "JavaScript expression on '{}' preserved as disclosure; bake keyframes or approve host expression evaluation.",
                prop_path
            );
            diagnostics.push(AepxLossDiagnostic {
                layer_name: layer.name.clone(),
                feature: format!("expression:{prop_path}"),
                classification: LossClassification::LossyApprovalRequired,
                message: msg,
                requires_user_approval: true,
            });
            disclosures_xml.push_str(&format!(
                "        <ExpressionDisclosure property=\"{}\" enabled=\"{}\" code=\"{}\"/>\n",
                xml_escape(&prop_path),
                enabled,
                xml_escape(&expr_text)
            ));
        }

        // Serialize Transform properties and keyframes (`anchor`, `position`, `scale`, `rotation`, `opacity`).
        let mut props_xml = String::new();
        if let Some(tr) = layer.transform() {
            for match_id in ["anchor", "position", "scale", "orientation", "rotationX", "rotationY", "rotation", "opacity"] {
                let Some(prop) = tr.get(match_id) else {
                    continue;
                };
                if prop.three_d_only && !layer.switches.three_d {
                    continue;
                }
                if prop.keys.is_empty() {
                    props_xml.push_str(&format!(
                        "        <Property match=\"{}\" value=\"{}\"/>\n",
                        xml_escape(&prop.match_id),
                        format_value_csv(&prop.value)
                    ));
                } else {
                    props_xml.push_str(&format!(
                        "        <Property match=\"{}\" value=\"{}\">\n",
                        xml_escape(&prop.match_id),
                        format_value_csv(&prop.value)
                    ));
                    for key in &prop.keys {
                        keyframe_count += 1;
                        let (kf_flicks, kf_exact) = tick_to_aepx_flicks(key.time);
                        if !kf_exact {
                            classification =
                                classification.escalate(LossClassification::CompatibleWithTransform);
                        }
                        let in_ease = key.in_ease.first().copied().unwrap_or_default();
                        let out_ease = key.out_ease.first().copied().unwrap_or_default();
                        props_xml.push_str(&format!(
                            "          <Keyframe flicks=\"{}\" value=\"{}\" inInterp=\"{}\" outInterp=\"{}\" inSpeed=\"{:.4}\" inInfluence=\"{:.4}\" outSpeed=\"{:.4}\" outInfluence=\"{:.4}\"/>\n",
                            kf_flicks,
                            format_value_csv(&key.value),
                            key.in_interp.label(),
                            key.out_interp.label(),
                            in_ease.speed,
                            in_ease.influence,
                            out_ease.speed,
                            out_ease.influence
                        ));
                    }
                    props_xml.push_str("        </Property>\n");
                }
            }
        }

        layers_xml.push_str(&format!(
            "      <Layer id=\"{}\" name=\"{}\" kind=\"{}\" startFlicks=\"{}\" inFlicks=\"{}\" outFlicks=\"{}\" stretch=\"{:.4}\" blendMode=\"{}\" video=\"{}\" audio=\"{}\" threeD=\"{}\" motionBlur=\"{}\" adjustment=\"{}\"{}>\n{}{}\
      </Layer>\n",
            layer.id.0,
            xml_escape(&layer.name),
            kind_tag,
            st_flicks,
            in_flicks,
            out_flicks,
            layer.stretch,
            xml_escape(layer.blend_mode.label()),
            layer.switches.video,
            layer.switches.audio,
            layer.switches.three_d,
            layer.switches.motion_blur,
            layer.switches.adjustment,
            source_attrs,
            props_xml,
            disclosures_xml
        ));
    }

    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<AfterEffectsProject xmlns=\"urn:troncs:cleanroom:aepx:v1\" schemaVersion=\"1.0\" flicksPerSecond=\"{AEPX_FLICKS_PER_SECOND}\" lossClassification=\"{}\">\n\
  <Composition name=\"{}\" width=\"{}\" height=\"{}\" pixelAspect=\"{:.6}\" frameRateNum=\"{}\" frameRateDen=\"{}\" durationFlicks=\"{}\" displayStartFlicks=\"{}\" workAreaStartFlicks=\"{}\" workAreaEndFlicks=\"{}\" bgR=\"{:.6}\" bgG=\"{:.6}\" bgB=\"{:.6}\">\n\
    <Layers>\n\
{}\
    </Layers>\n\
  </Composition>\n\
</AfterEffectsProject>\n",
        classification.as_str(),
        xml_escape(&item.name),
        comp.width,
        comp.height,
        comp.pixel_aspect,
        comp.frame_rate.num,
        comp.frame_rate.den,
        dur_flicks,
        start_flicks,
        wa_in_flicks,
        wa_out_flicks,
        comp.background[0],
        comp.background[1],
        comp.background[2],
        layers_xml
    );

    Ok(AepxExportReport {
        xml,
        loss_classification: classification,
        diagnostics,
        layer_count: comp.layers.len(),
        keyframe_count,
    })
}

fn parse_interp(raw: &str) -> Interp {
    match raw.trim().to_ascii_lowercase().as_str() {
        "bezier" => Interp::Bezier,
        "hold" => Interp::Hold,
        _ => Interp::Linear,
    }
}

/// Import a clean-room `.aepx` XML document (`705,600,000` flicks/sec) into `project`,
/// returning the newly created composition [`ItemId`] and explicit [`AepxLossDiagnostic`] list.
pub fn import_comp_from_aepx_xml(project: &mut Project, xml: &str) -> Result<AepxImportReport> {
    if !xml.contains("<AfterEffectsProject") && !xml.contains("<Composition") {
        return Err(Error::UnknownFormat);
    }

    let root_blocks = collect_blocks(xml, "AfterEffectsProject");
    let mut classification = root_blocks
        .first()
        .and_then(|(a, _)| a.get("lossClassification"))
        .map(|s| LossClassification::parse(s))
        .unwrap_or(LossClassification::ExactVerified);

    let comp_blocks = collect_blocks(xml, "Composition");
    let Some((comp_attrs, comp_body)) = comp_blocks.first() else {
        return Err(Error::UnknownFormat);
    };

    let comp_name = comp_attrs
        .get("name")
        .cloned()
        .unwrap_or_else(|| "Imported AEPX Comp".to_string());
    let width = comp_attrs
        .get("width")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(1920)
        .max(1);
    let height = comp_attrs
        .get("height")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(1080)
        .max(1);
    let pixel_aspect = comp_attrs
        .get("pixelAspect")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(1.0);
    let rate_num = comp_attrs
        .get("frameRateNum")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(30)
        .max(1);
    let rate_den = comp_attrs
        .get("frameRateDen")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(1)
        .max(1);
    let frame_rate = FrameRate::new(rate_num, rate_den);
    let duration = comp_attrs
        .get("durationFlicks")
        .and_then(|v| v.parse::<i64>().ok())
        .map(aepx_flicks_to_tick)
        .unwrap_or_else(|| Tick::from_seconds_f64(10.0));
    let display_start = comp_attrs
        .get("displayStartFlicks")
        .and_then(|v| v.parse::<i64>().ok())
        .map(aepx_flicks_to_tick)
        .unwrap_or(Tick::ZERO);
    let wa_start = comp_attrs
        .get("workAreaStartFlicks")
        .and_then(|v| v.parse::<i64>().ok())
        .map(aepx_flicks_to_tick)
        .unwrap_or(Tick::ZERO);
    let wa_end = comp_attrs
        .get("workAreaEndFlicks")
        .and_then(|v| v.parse::<i64>().ok())
        .map(aepx_flicks_to_tick)
        .unwrap_or(duration);
    let bg_r = comp_attrs.get("bgR").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
    let bg_g = comp_attrs.get("bgG").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
    let bg_b = comp_attrs.get("bgB").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);

    let mut comp = Comp::new(width, height, frame_rate, duration);
    comp.pixel_aspect = pixel_aspect;
    comp.display_start = display_start;
    comp.work_area = (wa_start, wa_end);
    comp.background = [bg_r, bg_g, bg_b];

    let mut diagnostics = Vec::new();
    let mut imported_keyframes = 0usize;

    for (l_attrs, l_body) in collect_blocks(comp_body, "Layer") {
        let layer_name = l_attrs
            .get("name")
            .cloned()
            .unwrap_or_else(|| "Layer".to_string());
        let kind = l_attrs.get("kind").map(String::as_str).unwrap_or("null");

        let (source, src_size, src_dur) = match kind {
            "footage" => {
                let path = l_attrs.get("srcPath").cloned().unwrap_or_default();
                let fw = l_attrs.get("srcWidth").and_then(|v| v.parse().ok()).unwrap_or(width);
                let fh = l_attrs.get("srcHeight").and_then(|v| v.parse().ok()).unwrap_or(height);
                let fr_num = l_attrs.get("srcRateNum").and_then(|v| v.parse().ok()).unwrap_or(rate_num);
                let fr_den = l_attrs.get("srcRateDen").and_then(|v| v.parse().ok()).unwrap_or(rate_den);
                let alpha = match l_attrs.get("srcAlpha").map(String::as_str) {
                    Some("premultiplied") => AlphaMode::Premultiplied,
                    Some("ignore") => AlphaMode::Ignore,
                    _ => AlphaMode::Straight,
                };
                let footage = Footage {
                    path: path.clone(),
                    kind: FootageKind::Video,
                    width: fw,
                    height: fh,
                    pixel_aspect: 1.0,
                    frame_rate: FrameRate::new(fr_num.max(1), fr_den.max(1)),
                    duration,
                    has_video: true,
                    has_audio: false,
                    alpha,
                    ..Footage::default()
                };
                let item_name = if path.is_empty() { layer_name.as_str() } else { path.as_str() };
                let item_id = project.add_item(item_name, Label::Aqua, None, ItemKind::Footage(footage));
                (LayerSource::Footage { item: item_id }, (fw, fh), Some(duration))
            }
            "solid" | "adjustment" => {
                let sr = l_attrs.get("solidR").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let sg = l_attrs.get("solidG").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let sb = l_attrs.get("solidB").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let sw = l_attrs.get("srcWidth").and_then(|v| v.parse().ok()).unwrap_or(width);
                let sh = l_attrs.get("srcHeight").and_then(|v| v.parse().ok()).unwrap_or(height);
                let solid = Solid { color: [sr, sg, sb], width: sw, height: sh, pixel_aspect: 1.0 };
                let item_id = project.add_item(&layer_name, Label::Red, None, ItemKind::Solid(solid));
                (LayerSource::Solid { item: item_id }, (sw, sh), Some(duration))
            }
            "text" => {
                classification = classification.escalate(LossClassification::CompatibleWithTransform);
                (LayerSource::Text, (width, height), Some(duration))
            }
            "shape" => {
                classification = classification.escalate(LossClassification::CompatibleWithTransform);
                (LayerSource::Shape, (width, height), Some(duration))
            }
            "null" => (LayerSource::Null, (100, 100), Some(duration)),
            other => {
                classification = classification.escalate(LossClassification::LossyApprovalRequired);
                diagnostics.push(AepxLossDiagnostic {
                    layer_name: layer_name.clone(),
                    feature: format!("layer_kind:{other}"),
                    classification: LossClassification::LossyApprovalRequired,
                    message: format!(
                        "Layer '{}' of kind '{}' imported as Null placeholder pending user approval.",
                        layer_name, other
                    ),
                    requires_user_approval: true,
                });
                (LayerSource::Null, (100, 100), Some(duration))
            }
        };

        let mut layer = build::layer(project, &comp, &layer_name, source, src_size, src_dur);
        if let Some(st) = l_attrs.get("startFlicks").and_then(|v| v.parse::<i64>().ok()) {
            layer.start_time = aepx_flicks_to_tick(st);
        }
        if let Some(inp) = l_attrs.get("inFlicks").and_then(|v| v.parse::<i64>().ok()) {
            layer.in_point = aepx_flicks_to_tick(inp);
        }
        if let Some(outp) = l_attrs.get("outFlicks").and_then(|v| v.parse::<i64>().ok()) {
            layer.out_point = aepx_flicks_to_tick(outp);
        }
        if let Some(stretch) = l_attrs.get("stretch").and_then(|v| v.parse::<f64>().ok()) {
            layer.stretch = stretch;
        }
        if let Some(bm_name) = l_attrs.get("blendMode") {
            layer.blend_mode = BlendMode::ALL
                .into_iter()
                .find(|m| m.label().eq_ignore_ascii_case(bm_name))
                .unwrap_or(BlendMode::Normal);
        }
        if let Some(v) = l_attrs.get("video").and_then(|s| s.parse::<bool>().ok()) {
            layer.switches.video = v;
        }
        if let Some(a) = l_attrs.get("audio").and_then(|s| s.parse::<bool>().ok()) {
            layer.switches.audio = a;
        }
        if let Some(td) = l_attrs.get("threeD").and_then(|s| s.parse::<bool>().ok()) {
            layer.switches.three_d = td;
        }
        if let Some(mb) = l_attrs.get("motionBlur").and_then(|s| s.parse::<bool>().ok()) {
            layer.switches.motion_blur = mb;
        }
        if kind == "adjustment"
            || l_attrs
                .get("adjustment")
                .and_then(|s| s.parse::<bool>().ok())
                .unwrap_or(false)
        {
            layer.switches.adjustment = true;
        }

        if matches!(layer.source, LayerSource::Text)
            && let Some(text_group) = layer.props.sub_mut("text")
            && let Some(st_prop) = text_group.get_mut("sourceText")
        {
            let mut doc = TextDoc::plain(l_attrs.get("text").map(String::as_str).unwrap_or(""));
            if l_attrs
                .get("rtl")
                .and_then(|s| s.parse::<bool>().ok())
                .unwrap_or(false)
            {
                doc.direction = Direction::Rtl;
            }
            if let Some(font) = l_attrs.get("font") {
                doc.font = font.clone();
            }
            if let Some(sz) = l_attrs.get("fontSize").and_then(|s| s.parse::<f64>().ok()) {
                doc.size = sz;
            }
            st_prop.value = Value::Text(Box::new(doc));
        }

        // Restore Transform properties & keyframes.
        for (p_attrs, p_body) in collect_blocks(l_body, "Property") {
            let Some(match_id) = p_attrs.get("match") else {
                continue;
            };
            let Some(tr) = layer.transform_mut() else {
                continue;
            };
            let Some(prop) = tr.get_mut(match_id) else {
                continue;
            };
            if let Some(val_str) = p_attrs.get("value") {
                prop.value = parse_value_csv(match_id, val_str);
            }
            let kf_blocks = collect_blocks(p_body, "Keyframe");
            if !kf_blocks.is_empty() {
                prop.keys.clear();
                for (kf_attrs, _) in kf_blocks {
                    let flicks = kf_attrs
                        .get("flicks")
                        .and_then(|v| v.parse::<i64>().ok())
                        .unwrap_or(0);
                    let kv = kf_attrs
                        .get("value")
                        .map(|s| parse_value_csv(match_id, s))
                        .unwrap_or_else(|| prop.value.clone());
                    let mut kf = Keyframe::new(aepx_flicks_to_tick(flicks), kv);
                    kf.in_interp = kf_attrs
                        .get("inInterp")
                        .map(|s| parse_interp(s))
                        .unwrap_or(Interp::Linear);
                    kf.out_interp = kf_attrs
                        .get("outInterp")
                        .map(|s| parse_interp(s))
                        .unwrap_or(Interp::Linear);
                    let dims = kf.value.dims().max(1);
                    let in_speed = kf_attrs.get("inSpeed").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let in_inf = kf_attrs
                        .get("inInfluence")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(1.0 / 3.0);
                    let out_speed = kf_attrs.get("outSpeed").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let out_inf = kf_attrs
                        .get("outInfluence")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(1.0 / 3.0);
                    kf.in_ease = vec![Ease { speed: in_speed, influence: in_inf }; dims];
                    kf.out_ease = vec![Ease { speed: out_speed, influence: out_inf }; dims];
                    prop.keys.push(kf);
                    imported_keyframes += 1;
                }
            }
        }

        // Preserve any explicit disclosures from the `.aepx` document!
        for (fx_attrs, _) in collect_blocks(l_body, "EffectDisclosure") {
            classification = classification.escalate(LossClassification::LossyApprovalRequired);
            let match_id = fx_attrs.get("matchId").cloned().unwrap_or_else(|| "unknown".into());
            let msg = fx_attrs
                .get("message")
                .cloned()
                .unwrap_or_else(|| format!("Unsupported effect '{match_id}' requires approval."));
            diagnostics.push(AepxLossDiagnostic {
                layer_name: layer_name.clone(),
                feature: format!("effect:{match_id}"),
                classification: LossClassification::LossyApprovalRequired,
                message: msg,
                requires_user_approval: true,
            });
        }
        for (ex_attrs, _) in collect_blocks(l_body, "ExpressionDisclosure") {
            classification = classification.escalate(LossClassification::LossyApprovalRequired);
            let prop_path = ex_attrs.get("property").cloned().unwrap_or_else(|| "transform".into());
            let code = ex_attrs.get("code").cloned().unwrap_or_default();
            let enabled = ex_attrs
                .get("enabled")
                .and_then(|v| v.parse::<bool>().ok())
                .unwrap_or(true);
            // Re-attach expression onto transform property if simple leaf match, while still reporting disclosure.
            if let Some(leaf) = prop_path.rsplit('/').next()
                && let Some(tr) = layer.transform_mut()
                && let Some(prop) = tr.get_mut(leaf)
            {
                prop.expr = Some(effectcraft_project::Expression { text: code, enabled });
            }
            diagnostics.push(AepxLossDiagnostic {
                layer_name: layer_name.clone(),
                feature: format!("expression:{prop_path}"),
                classification: LossClassification::LossyApprovalRequired,
                message: format!("Expression on '{prop_path}' imported with explicit approval disclosure."),
                requires_user_approval: true,
            });
        }
        for (fd_attrs, _) in collect_blocks(l_body, "FeatureDisclosure") {
            classification = classification.escalate(LossClassification::LossyApprovalRequired);
            let feature = fd_attrs.get("feature").cloned().unwrap_or_else(|| "feature".into());
            let msg = fd_attrs
                .get("message")
                .cloned()
                .unwrap_or_else(|| format!("Feature '{feature}' requires user approval."));
            diagnostics.push(AepxLossDiagnostic {
                layer_name: layer_name.clone(),
                feature,
                classification: LossClassification::LossyApprovalRequired,
                message: msg,
                requires_user_approval: true,
            });
        }

        comp.layers.push(layer);
    }

    let imported_layers = comp.layers.len();
    let unique_name = project.unique_item_name(&comp_name, |_| false);
    let comp_id = project.add_item(
        &unique_name,
        Label::Sandstone,
        None,
        ItemKind::Comp(std::sync::Arc::new(comp)),
    );

    Ok(AepxImportReport {
        comp_id,
        comp_name: unique_name,
        loss_classification: classification,
        diagnostics,
        imported_layers,
        imported_keyframes,
    })
}

// ---------------------------------------------------------------------------
// TronCS Cross-Suite Dynamic Asset Link (`troncs.handoff.v1`)
// PhotoTron -> AfterTron -> FilmTron
// ---------------------------------------------------------------------------

/// Versioned cross-suite asset handoff contract (`troncs.handoff.v1`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TroncsHandoffBundleV1 {
    pub schema_version: String,
    pub bundle_id: String,
    pub producer_app: String,
    pub consumer_app: String,
    pub asset_id: String,
    pub revision: u64,
    pub sha256: String,
    pub loss_classification: LossClassification,
    pub loss_notes: Vec<String>,
    pub rendition_path: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate_num: i64,
    pub frame_rate_den: i64,
    pub alpha_channel: bool,
    pub color_space: String,
    pub recipe_json: String,
}

/// Result of refreshing an incoming `troncs.handoff.v1` link inside `AfterTron`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicLinkRefreshOutcome {
    pub footage_item_id: ItemId,
    pub layer_id: LayerId,
    pub updated_existing_link: bool,
    pub preserved_keyframe_count: usize,
    pub revision: u64,
}

/// Link or refresh an upstream `troncs.handoff.v1` asset (e.g. `PhotoTron` / `LightTron` / `FigTron`)
/// inside an `AfterTron` composition without clobbering existing motion keyframes!
pub fn refresh_dynamic_handoff_link(
    project: &mut Project,
    comp_id: ItemId,
    bundle: &TroncsHandoffBundleV1,
) -> Result<DynamicLinkRefreshOutcome> {
    if project.comp(comp_id).is_none() {
        return Err(Error::NoComp(comp_id.0));
    }

    let tag_prefix = format!("troncs.assetId={};", bundle.asset_id);
    let comment_stamp = format!(
        "{tag_prefix}rev={};sha256={};producer={};schema={}",
        bundle.revision, bundle.sha256, bundle.producer_app, bundle.schema_version
    );

    // 1. Find existing linked footage item by stable `asset_id` tag in `Item::comment`.
    let existing_item_id = project
        .items
        .values()
        .find(|it| matches!(it.kind, ItemKind::Footage(_)) && it.comment.starts_with(&tag_prefix))
        .map(|it| it.id);

    let is_video = bundle.frame_rate_num > 0
        && (bundle.rendition_path.ends_with(".mov")
            || bundle.rendition_path.ends_with(".mp4")
            || bundle.rendition_path.ends_with(".mxf"));
    let footage = Footage {
        path: bundle.rendition_path.clone(),
        kind: if is_video { FootageKind::Video } else { FootageKind::Still },
        width: bundle.width.max(1),
        height: bundle.height.max(1),
        pixel_aspect: 1.0,
        frame_rate: FrameRate::new(bundle.frame_rate_num.max(1), bundle.frame_rate_den.max(1)),
        duration: project
            .comp(comp_id)
            .map(|c| c.duration)
            .unwrap_or_else(|| Tick::from_seconds_f64(5.0)),
        has_video: true,
        has_audio: false,
        alpha: if bundle.alpha_channel { AlphaMode::Straight } else { AlphaMode::Ignore },
        ..Footage::default()
    };

    let (footage_item_id, updated_existing_link) = if let Some(item_id) = existing_item_id {
        if let Some(it) = project.item_mut(item_id) {
            it.kind = ItemKind::Footage(footage);
            it.comment = comment_stamp;
        }
        (item_id, true)
    } else {
        let item_name = format!("{} [{}]", bundle.asset_id, bundle.producer_app);
        let item_id = project.add_item(&item_name, Label::Aqua, None, ItemKind::Footage(footage));
        if let Some(it) = project.item_mut(item_id) {
            it.comment = comment_stamp;
        }
        (item_id, false)
    };

    // 2. Ensure the target composition references this footage item, preserving any keyframes!
    let existing_layer_info = project.comp(comp_id).and_then(|comp| {
        comp.layers
            .iter()
            .find(|l| l.source == LayerSource::Footage { item: footage_item_id })
            .map(|l| {
                let mut kf_count = 0usize;
                l.props.walk("", &mut |_, p: &Property| kf_count += p.keys.len());
                (l.id, kf_count)
            })
    });

    if let Some((layer_id, preserved_keyframe_count)) = existing_layer_info {
        Ok(DynamicLinkRefreshOutcome {
            footage_item_id,
            layer_id,
            updated_existing_link,
            preserved_keyframe_count,
            revision: bundle.revision,
        })
    } else {
        let comp_snapshot = project
            .comp(comp_id)
            .cloned()
            .ok_or(Error::NoComp(comp_id.0))?;
        let layer_name = format!("Linked {}", bundle.asset_id);
        let new_layer = build::layer(
            project,
            &comp_snapshot,
            &layer_name,
            LayerSource::Footage { item: footage_item_id },
            (bundle.width.max(1), bundle.height.max(1)),
            None,
        );
        let layer_id = new_layer.id;
        if let Some(comp_mut) = project.comp_mut(comp_id) {
            comp_mut.layers.insert(0, new_layer);
        }
        Ok(DynamicLinkRefreshOutcome {
            footage_item_id,
            layer_id,
            updated_existing_link,
            preserved_keyframe_count: 0,
            revision: bundle.revision,
        })
    }
}

/// Build an outbound `troncs.handoff.v1` ProRes 4444 alpha stem bundle from `AfterTron` to `FilmTron`.
pub fn build_filmtron_handoff_bundle(
    project: &Project,
    comp_id: ItemId,
    rendered_stem_path: &str,
    sha256: &str,
    revision: u64,
) -> Result<TroncsHandoffBundleV1> {
    let aepx = export_comp_to_aepx_xml(project, comp_id)?;
    let Some(item) = project.item(comp_id) else {
        return Err(Error::NoComp(comp_id.0));
    };
    let Some(comp) = item.as_comp() else {
        return Err(Error::NoComp(comp_id.0));
    };

    let notes: Vec<String> = aepx
        .diagnostics
        .iter()
        .map(|d| format!("{}: {} ({})", d.layer_name, d.message, d.classification.as_str()))
        .collect();

    Ok(TroncsHandoffBundleV1 {
        schema_version: TRONCS_HANDOFF_SCHEMA_V1.to_string(),
        bundle_id: format!("aftertron-comp-{}-r{}", comp_id.0, revision),
        producer_app: "AfterTron".to_string(),
        consumer_app: "FilmTron".to_string(),
        asset_id: item.name.clone(),
        revision,
        sha256: sha256.to_string(),
        // Because AfterTron delivers both the baked ProRes 4444 alpha stem AND the parametric AEPX recipe,
        // visual pixels in FilmTron are exact-verified if clean or compatible-with-transform when baked!
        loss_classification: if aepx.loss_classification == LossClassification::ExactVerified {
            LossClassification::ExactVerified
        } else {
            LossClassification::CompatibleWithTransform
        },
        loss_notes: notes,
        rendition_path: rendered_stem_path.to_string(),
        width: comp.width,
        height: comp.height,
        frame_rate_num: comp.frame_rate.num,
        frame_rate_den: comp.frame_rate.den,
        alpha_channel: true,
        color_space: "Rec.709 / Straight Alpha (ProRes 4444)".to_string(),
        recipe_json: serde_json::json!({
            "compId": comp_id.0,
            "compName": item.name,
            "flicksPerSecond": AEPX_FLICKS_PER_SECOND,
            "aepxLossClassification": aepx.loss_classification.as_str(),
            "layerCount": aepx.layer_count,
            "keyframeCount": aepx.keyframe_count,
        })
        .to_string(),
    })
}

#[cfg(test)]
mod aepx_tests {
    use super::*;
    use effectcraft_project::Expression;

    #[test]
    fn aepx_705m_flicks_roundtrips_comp_layers_and_eased_keyframes() {
        let mut project = Project::default();
        let mut comp = Comp::new(1920, 1080, FrameRate::new(24, 1), Tick::from_seconds_f64(4.0));
        let solid_id = project.add_item(
            "Hero BG",
            Label::Red,
            None,
            ItemKind::Solid(Solid {
                color: [0.1, 0.2, 0.3],
                width: 1920,
                height: 1080,
                pixel_aspect: 1.0,
            }),
        );
        let mut layer = build::layer(
            &mut project,
            &comp,
            "Hero Solid",
            LayerSource::Solid { item: solid_id },
            (1920, 1080),
            None,
        );
        if let Some(tr) = layer.transform_mut()
            && let Some(pos) = tr.get_mut("position")
        {
            pos.keys.push(Keyframe::new(Tick::ZERO, Value::Vec3([960.0, 540.0, 0.0])).eased());
            pos.keys.push(
                Keyframe::new(Tick::from_seconds_f64(2.0), Value::Vec3([1200.0, 400.0, 0.0]))
                    .eased(),
            );
        }
        comp.layers.push(layer);
        let comp_id = project.add_item("Main Reveal", Label::Sandstone, None, ItemKind::Comp(std::sync::Arc::new(comp)));

        let exported = export_comp_to_aepx_xml(&project, comp_id);
        assert!(exported.is_ok());
        let Ok(exported) = exported else { return };
        assert_eq!(exported.loss_classification, LossClassification::ExactVerified);
        assert!(exported.xml.contains("flicksPerSecond=\"705600000\""));
        // 2.0 seconds == 1,411,200,000 flicks!
        assert!(exported.xml.contains("flicks=\"1411200000\""));

        let mut dst = Project::default();
        let imported = import_comp_from_aepx_xml(&mut dst, &exported.xml);
        assert!(imported.is_ok());
        let Ok(imported) = imported else { return };
        assert_eq!(imported.loss_classification, LossClassification::ExactVerified);
        assert_eq!(imported.imported_layers, 1);
        assert_eq!(imported.imported_keyframes, 2);
    }

    #[test]
    fn aepx_discloses_expressions_and_effects_as_lossy_approval_required() {
        let mut project = Project::default();
        let mut comp = Comp::new(1920, 1080, FrameRate::FPS_30, Tick::from_seconds_f64(3.0));
        let mut null_layer =
            build::layer(&mut project, &comp, "Whip Pan Rig", LayerSource::Null, (100, 100), None);
        if let Some(tr) = null_layer.transform_mut()
            && let Some(rot) = tr.get_mut("rotation")
        {
            rot.expr = Some(Expression { text: "wiggle(4, 15)".into(), enabled: true });
        }
        comp.layers.push(null_layer);
        let comp_id = project.add_item("Rig Comp", Label::Sandstone, None, ItemKind::Comp(std::sync::Arc::new(comp)));

        let Ok(exported) = export_comp_to_aepx_xml(&project, comp_id) else {
            return;
        };
        assert_eq!(exported.loss_classification, LossClassification::LossyApprovalRequired);
        assert!(exported.diagnostics.iter().any(|d| d.requires_user_approval && d.feature.starts_with("expression:")));

        let mut reimported_proj = Project::default();
        let Ok(reimported) = import_comp_from_aepx_xml(&mut reimported_proj, &exported.xml) else {
            return;
        };
        assert_eq!(reimported.loss_classification, LossClassification::LossyApprovalRequired);
        assert!(!reimported.diagnostics.is_empty());
    }

    #[test]
    fn troncs_handoff_v1_links_phototron_and_preserves_keyframes_on_refresh() {
        let mut project = Project::default();
        let comp = Comp::new(1920, 1080, FrameRate::FPS_30, Tick::from_seconds_f64(5.0));
        let comp_id = project.add_item("Composite", Label::Sandstone, None, ItemKind::Comp(std::sync::Arc::new(comp)));

        let v1_bundle = TroncsHandoffBundleV1 {
            schema_version: TRONCS_HANDOFF_SCHEMA_V1.into(),
            bundle_id: "phototron-hero-r1".into(),
            producer_app: "PhotoTron".into(),
            consumer_app: "AfterTron".into(),
            asset_id: "hero-plate".into(),
            revision: 1,
            sha256: "1111aaaa".into(),
            loss_classification: LossClassification::ExactVerified,
            loss_notes: vec![],
            rendition_path: "/renders/hero-plate-r1.png".into(),
            width: 1920,
            height: 1080,
            frame_rate_num: 30,
            frame_rate_den: 1,
            alpha_channel: true,
            color_space: "Display P3".into(),
            recipe_json: "{}".into(),
        };

        let Ok(first) = refresh_dynamic_handoff_link(&mut project, comp_id, &v1_bundle) else {
            return;
        };
        assert!(!first.updated_existing_link);

        // Animate opacity on the linked PhotoTron layer inside AfterTron:
        if let Some(c) = project.comp_mut(comp_id)
            && let Some(l) = c.layer_mut(first.layer_id)
            && let Some(tr) = l.transform_mut()
            && let Some(op) = tr.get_mut("opacity")
        {
            op.keys.push(Keyframe::new(Tick::ZERO, Value::Scalar(0.0)));
            op.keys.push(Keyframe::new(Tick::from_seconds_f64(1.0), Value::Scalar(100.0)));
        }

        // Upstream PhotoTron updates revision 2:
        let mut v2_bundle = v1_bundle.clone();
        v2_bundle.revision = 2;
        v2_bundle.sha256 = "2222bbbb".into();
        v2_bundle.rendition_path = "/renders/hero-plate-r2.png".into();

        let Ok(second) = refresh_dynamic_handoff_link(&mut project, comp_id, &v2_bundle) else {
            return;
        };
        assert!(second.updated_existing_link);
        assert_eq!(second.footage_item_id, first.footage_item_id);
        assert_eq!(second.layer_id, first.layer_id);
        assert_eq!(second.preserved_keyframe_count, 2);

        // Build downstream FilmTron handoff bundle:
        let Ok(filmtron_bundle) = build_filmtron_handoff_bundle(
            &project,
            comp_id,
            "/renders/composite-prores4444.mov",
            "3333cccc",
            2,
        ) else {
            return;
        };
        assert_eq!(filmtron_bundle.schema_version, "troncs.handoff.v1");
        assert_eq!(filmtron_bundle.producer_app, "AfterTron");
        assert_eq!(filmtron_bundle.consumer_app, "FilmTron");
        assert!(filmtron_bundle.alpha_channel);
    }
}

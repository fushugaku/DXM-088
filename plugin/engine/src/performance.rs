use super::*;
use lxr02::performance::{Route, Scene, ROUTES};

pub fn encode(data: &[f32]) -> String {
    const ALPH: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes: Vec<u8> = data
        .iter()
        .flat_map(|v| ((v.clamp(-1., 1.) * 32767.).round() as i16).to_le_bytes())
        .collect();
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let a = c[0] as u32;
        let b = c.get(1).copied().unwrap_or(0) as u32;
        let d = c.get(2).copied().unwrap_or(0) as u32;
        let n = (a << 16) | (b << 8) | d;
        out.push(ALPH[(n >> 18) as usize] as char);
        out.push(ALPH[((n >> 12) & 63) as usize] as char);
        out.push(if c.len() > 1 {
            ALPH[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            ALPH[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
fn decode(text: &str) -> Option<Vec<f32>> {
    if text.len() > 2_560_000 || text.len() % 4 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(text.len() / 4 * 3);
    let val = |x: u8| -> Option<u32> {
        Some(match x {
            b'A'..=b'Z' => (x - b'A') as u32,
            b'a'..=b'z' => (x - b'a' + 26) as u32,
            b'0'..=b'9' => (x - b'0' + 52) as u32,
            b'+' => 62,
            b'/' => 63,
            b'=' => 0,
            _ => return None,
        })
    };
    for (i, c) in text.as_bytes().chunks(4).enumerate() {
        if c[0] == b'='
            || c[1] == b'='
            || (c[2] == b'=' && c[3] != b'=')
            || (i + 1 < text.len() / 4 && c.contains(&b'='))
        {
            return None;
        }
        let n = (val(c[0])? << 18) | (val(c[1])? << 12) | (val(c[2])? << 6) | val(c[3])?;
        bytes.push((n >> 16) as u8);
        if c[2] != b'=' {
            bytes.push((n >> 8) as u8);
        }
        if c[3] != b'=' {
            bytes.push(n as u8);
        }
    }
    if bytes.len() % 2 != 0 || bytes.len() / 2 > 960000 {
        return None;
    }
    Some(
        bytes
            .chunks_exact(2)
            .map(|b| (i16::from_le_bytes([b[0], b[1]]) as f32 / 32767.).clamp(-1., 1.))
            .collect(),
    )
}
fn num(x: &Value, min: f64, max: f64) -> Option<f32> {
    let n = x.as_f64()?;
    (n.is_finite() && n >= min - 0.00001 && n <= max + 0.00001).then_some(n.clamp(min, max) as f32)
}
fn values<const N: usize>(x: &Value, min: f64, max: f64) -> Option<[f32; N]> {
    if x.as_array()?.len() != N {
        return None;
    }
    let mut out = [0.; N];
    for i in 0..N {
        out[i] = num(&x[i], min, max)?;
    }
    Some(out)
}
fn scene_json(s: &Scene) -> Value {
    json!({"enabled":s.enabled,"params":s.params.map(|p|p.to_vec()),"fx":s.fx,"levels":s.levels,"muted":s.muted,"solo":s.solo,"macros":s.macros,"morph":s.morph,"morphTarget":s.morph_target})
}
impl Instrument {
    pub(super) fn extensions(&self, j: &mut Value) {
        let e = &self.engine;
        j["performance"] = json!({"macros":e.global_macros,"macroNames":self.macro_names,"routes":e.routes.iter().map(|r|json!({"source":r.source,"voice":r.voice,"param":r.param,"amount":r.amount,"sourceVoice":r.source_voice,"sourceParam":r.source_param,"configured":r.configured})).collect::<Vec<_>>(),"velocityDepth":e.velocity_depth,"switchQuant":e.switch_quant,"lfoPeriods":e.lfo_periods,"mutation":self.mutation});
        j["scenes"] = json!(e
            .scenes
            .iter()
            .map(|s| if s.enabled {
                scene_json(s)
            } else {
                Value::Null
            })
            .collect::<Vec<_>>());
        j["samples"]=json!(e.samples.iter().enumerate().map(|(t,s)|if s.pcm.is_empty(){Value::Null}else{json!({"name":self.sample_names[t],"rate":s.rate,"data":encode(&s.pcm),"gain":s.gain,"tune":s.tune,"start":s.start,"end":s.end,"decay":s.decay,"replace":s.replace})}).collect::<Vec<_>>());
    }
    pub(super) fn load_extensions(&mut self, j: &Value) -> Option<()> {
        if j["version"] == 2 {
            return Some(());
        }
        let perf = &j["performance"];
        if perf["mutation"].is_object() {
            self.mutation = perf["mutation"].clone();
        }
        self.engine.macros = values(&perf["macros"], 0., 1.)?;
        self.engine.global_macros = self.engine.macros;
        self.engine.lfo_periods = values(&perf["lfoPeriods"], 0.0625, 32.)?;
        self.engine.velocity_depth = num(&perf["velocityDepth"], 0., 1.)?;
        self.engine.switch_quant = num(&perf["switchQuant"], 0., 2.)? as u8;
        if perf["routes"].as_array()?.len() != ROUTES || perf["macroNames"].as_array()?.len() != 4 {
            return None;
        }
        for i in 0..4 {
            self.macro_names[i] = perf["macroNames"][i].as_str()?.chars().take(20).collect();
        }
        for i in 0..ROUTES {
            let r = &perf["routes"][i];
            self.engine.routes[i] = Route {
                source: num(&r["source"], 0., 12.)? as u8,
                voice: num(&r["voice"], 0., 5.)? as u8,
                param: num(&r["param"], 0., (PARAMS - 1) as f64)? as u8,
                amount: num(&r["amount"], -1., 1.)?,
                source_voice: if r.get("sourceVoice").is_some() {
                    num(&r["sourceVoice"], 0., 6.)? as u8
                } else {
                    num(&r["voice"], 0., 5.)? as u8
                },
                source_param: if r.get("sourceParam").is_some() {
                    num(&r["sourceParam"], 0., (PARAMS - 1) as f64)? as u8
                } else {
                    12
                },
                configured: r
                    .get("configured")
                    .map(|x| x.as_bool())
                    .unwrap_or(Some(r["amount"].as_f64()? != 0.))?,
            };
        }
        let count = j["patterns"].as_array()?.len();
        for p in 0..count {
            let pattern = &j["patterns"][p];
            for key in ["divisions", "delays", "grooves", "grooveAmounts"] {
                if pattern[key].as_array()?.len() != 7 {
                    return None;
                }
            }
            for t in 0..7 {
                self.engine.track_settings(
                    p,
                    t,
                    num(&pattern["divisions"][t], 0., 12.)? as u8,
                    num(&pattern["delays"][t], -0.49, 0.49)?,
                    num(&pattern["grooves"][t], 0., 4.)? as u8,
                    num(&pattern["grooveAmounts"][t], 0., 1.)?,
                );
                if let Some(swings) = pattern.get("swings") {
                    if swings.as_array()?.len() != 7 {
                        return None;
                    }
                    self.engine.track_swing(p, t, num(&swings[t], 0., 0.5)?);
                }
                for k in 0..64 {
                    let s = &pattern["tracks"][t][k];
                    self.engine.step_extras(
                        p,
                        t,
                        k,
                        num(&s["micro"], -0.49, 0.49)?,
                        num(&s["condition"], 0., 7.)? as u8,
                        num(&s["every"], 1., 16.)? as u8,
                        num(&s["phase"], 0., 15.)? as u8,
                    );
                }
            }
        }
        if j["scenes"].as_array()?.len() != count || j["samples"].as_array()?.len() != 7 {
            return None;
        }
        for p in 0..count {
            let x = &j["scenes"][p];
            if x.is_null() {
                continue;
            }
            let mut s = Scene::default();
            s.enabled = x["enabled"].as_bool()?;
            if x["params"].as_array()?.len() != 6 || x["fx"].as_array()?.len() != 15 {
                return None;
            }
            for v in 0..6 {
                let size = x["params"][v].as_array()?.len();
                if size != LEGACY_PARAMS && size != PARAMS {
                    return None;
                }
                for id in 0..size {
                    s.params[v][id] = num(&x["params"][v][id], MIN[id] as f64, MAX[id] as f64)?;
                }
            }
            for id in 0..15 {
                s.fx[id] = num(&x["fx"][id], FX_MIN[id] as f64, FX_MAX[id] as f64)?;
            }
            s.levels = values(&x["levels"], 0., 1.25)?;
            for key in ["muted", "solo"] {
                if x[key].as_array()?.len() != 7 {
                    return None;
                }
            }
            for t in 0..7 {
                s.muted[t] = x["muted"][t].as_bool()?;
                s.solo[t] = x["solo"][t].as_bool()?;
            }
            s.macros = values(&x["macros"], 0., 1.)?;
            s.morph = num(&x["morph"], 0., 1.)?;
            s.morph_target = num(&x["morphTarget"], 0., 4.)? as usize;
            self.engine.scenes[p] = s;
        }
        for t in 0..7 {
            let x = &j["samples"][t];
            if x.is_null() {
                continue;
            }
            self.sample_names[t] = x["name"].as_str()?.chars().take(100).collect();
            self.engine.sample_data(
                t,
                num(&x["rate"], 8000., 192000.)?,
                decode(x["data"].as_str()?)?,
            );
            self.engine.sample_settings(
                t,
                num(&x["gain"], 0., 1.5)?,
                num(&x["tune"], -36., 36.)?,
                num(&x["start"], 0., 0.99)?,
                num(&x["end"], 0.001, 1.)?,
                num(&x["decay"], 0.005, 10.)?,
                x["replace"].as_bool()?,
            );
            if x["end"].as_f64()? <= x["start"].as_f64()? {
                return None;
            }
        }
        self.engine.immediate_pattern(self.engine.pattern);
        Some(())
    }
}

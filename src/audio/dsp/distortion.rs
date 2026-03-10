use super::{Effect, EffectParameter};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DistortionType {
    HardClip,
    SoftClip,
    Foldback,
    Waveshape,
}

pub struct Distortion {
    drive: f32,          // 0.0-1.0
    dist_type: DistortionType,
}

impl Distortion {
    pub fn new() -> Self {
        Distortion {
            drive: 0.5,
            dist_type: DistortionType::SoftClip,
        }
    }
}

impl Effect for Distortion {
    fn process(&mut self, input: f32) -> f32 {
        let driven = input * (1.0 + self.drive * 20.0);
        match self.dist_type {
            DistortionType::HardClip => driven.clamp(-1.0, 1.0),
            DistortionType::SoftClip => {
                // Tanh-based soft clipping
                driven.tanh()
            }
            DistortionType::Foldback => {
                // Foldback distortion
                let mut s = driven;
                while s > 1.0 || s < -1.0 {
                    if s > 1.0 { s = 2.0 - s; }
                    if s < -1.0 { s = -2.0 - s; }
                }
                s
            }
            DistortionType::Waveshape => {
                // Cubic waveshaping
                let x = driven.clamp(-1.5, 1.5);
                x - (x * x * x) / 3.0
            }
        }
    }

    fn set_parameter(&mut self, param_name: &str, value: f32) {
        match param_name {
            "drive" => self.drive = value.clamp(0.0, 1.0),
            "type" => {
                self.dist_type = match value as u8 {
                    0 => DistortionType::HardClip,
                    1 => DistortionType::SoftClip,
                    2 => DistortionType::Foldback,
                    3 => DistortionType::Waveshape,
                    _ => DistortionType::SoftClip,
                };
            }
            _ => {}
        }
    }

    fn get_parameters(&self) -> Vec<EffectParameter> {
        vec![
            EffectParameter::new("drive", self.drive, 0.0, 1.0),
            EffectParameter::new("type", self.dist_type as u8 as f32, 0.0, 3.0),
        ]
    }

    fn name(&self) -> &str {
        "Distortion"
    }
}

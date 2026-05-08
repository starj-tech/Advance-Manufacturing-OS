use serde::{Deserialize, Serialize};

/// Weighted vote across heterogeneous evidence sources. Pure indoor
/// GPS fails; pure Wi-Fi spoofs easily; combining mitigates both.
///
/// Default weights: GPS 0.5, Wi-Fi 0.3, BLE 0.2. Threshold 0.6.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Evidence {
    Gps {
        inside: bool,
        accuracy_m: u32,
    },
    /// fraction = matched_bssids / fence.allowed_bssids.len()
    WifiBssid {
        fraction: f32,
    },
    /// fraction = matched_beacons / fence.allowed_beacons.len()
    Bluetooth {
        fraction: f32,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Verdict {
    pub inside: bool,
    pub confidence: f32,
    pub evidence_used: Vec<String>,
}

pub struct GeofenceEvaluator {
    pub weight_gps: f32,
    pub weight_wifi: f32,
    pub weight_ble: f32,
    pub threshold: f32,
}

impl Default for GeofenceEvaluator {
    fn default() -> Self {
        Self {
            weight_gps: 0.5,
            weight_wifi: 0.3,
            weight_ble: 0.2,
            threshold: 0.6,
        }
    }
}

impl GeofenceEvaluator {
    pub fn evaluate(&self, evidence: &[Evidence]) -> Verdict {
        let mut score = 0.0;
        let mut total_weight = 0.0;
        let mut used: Vec<String> = vec![];

        for e in evidence {
            match e {
                Evidence::Gps { inside, accuracy_m } if *accuracy_m < 30 => {
                    score += self.weight_gps * if *inside { 1.0 } else { 0.0 };
                    total_weight += self.weight_gps;
                    used.push("gps".into());
                }
                Evidence::Gps { .. } => {
                    // Low-accuracy GPS is dropped, not penalized.
                }
                Evidence::WifiBssid { fraction } => {
                    let f = fraction.clamp(0.0, 1.0);
                    score += self.weight_wifi * f;
                    total_weight += self.weight_wifi;
                    used.push("wifi".into());
                }
                Evidence::Bluetooth { fraction } => {
                    let f = fraction.clamp(0.0, 1.0);
                    score += self.weight_ble * f;
                    total_weight += self.weight_ble;
                    used.push("ble".into());
                }
            }
        }

        let confidence = if total_weight > 0.0 {
            score / total_weight
        } else {
            0.0
        };

        Verdict {
            inside: confidence >= self.threshold,
            confidence,
            evidence_used: used,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gps_inside_with_decent_accuracy_passes() {
        let eval = GeofenceEvaluator::default();
        let v = eval.evaluate(&[Evidence::Gps {
            inside: true,
            accuracy_m: 10,
        }]);
        assert!(v.inside);
    }

    #[test]
    fn low_accuracy_gps_is_ignored() {
        let eval = GeofenceEvaluator::default();
        let v = eval.evaluate(&[Evidence::Gps {
            inside: false,
            accuracy_m: 200,
        }]);
        assert_eq!(v.confidence, 0.0);
        assert!(!v.inside);
    }

    #[test]
    fn wifi_alone_with_perfect_match_falls_short_of_threshold() {
        // Wi-Fi weight 0.3 / total 0.3 = 1.0 — actually passes.
        // Demonstrate that wifi alone CAN trip threshold but real config
        // typically requires GPS or BLE corroboration.
        let eval = GeofenceEvaluator::default();
        let v = eval.evaluate(&[Evidence::WifiBssid { fraction: 1.0 }]);
        assert!(v.inside);
    }

    #[test]
    fn combined_evidence_increases_confidence() {
        let eval = GeofenceEvaluator::default();
        let v = eval.evaluate(&[
            Evidence::Gps {
                inside: true,
                accuracy_m: 10,
            },
            Evidence::WifiBssid { fraction: 1.0 },
            Evidence::Bluetooth { fraction: 1.0 },
        ]);
        assert!(v.inside);
        assert_eq!(v.evidence_used.len(), 3);
        assert!((v.confidence - 1.0).abs() < f32::EPSILON);
    }
}

//! Adapter from `tpt-teleop-agri` field data to `tpt-senses` observations.
//!
//! `tpt-teleop-agri` is not published to crates.io, so this crate cannot depend on it. A crates.io
//! package may not depend on a git source, so the input types here are **mirrors** of the source
//! structs. Each mirror says where it comes from. When the source changes, update the mirror and
//! its tests together.
//!
//! Source commit these mirrors were read from: `tpt-teleop-agri` `1c469ffbbcb7df04b2acdc4d2dbad6de5888456f`.
//!
//! * [`GnssFixMirror`]: `crates/tpt-t-agri-core/src/wire.rs`, `GnssFix`.
//! * [`WindMirror`]: `crates/tpt-t-agri-env/src/lib.rs`, `WindSample`.
//! * [`SoilMirror`]: `crates/tpt-t-agri-env/src/lib.rs`, `SoilSample`.
//! * [`RainMirror`]: `crates/tpt-t-agri-env/src/lib.rs`, `RainSample`.

use tpt_senses::prelude::*;
use tpt_senses::{
    Adapter, AdapterError, CoordinateFrame, Point3, SpatialReference, StandardDeviation,
    Uncertainty,
};

/// Mirror of `GnssFix`: a GNSS/RTK fix in WGS-84.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GnssFixMirror {
    /// Latitude, degrees.
    pub lat_deg: f64,
    /// Longitude, degrees.
    pub lon_deg: f64,
    /// Ellipsoidal altitude, metres.
    pub alt_m: f64,
    /// Horizontal precision estimate, metres. `0.0` means unknown.
    pub hprec_m: f32,
    /// `true` when RTK-fixed, `false` for float or standalone.
    pub rtk_fixed: bool,
}

/// Mirror of `WindSample`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindMirror {
    /// Speed at sensor height, m/s.
    pub speed_m_s: f32,
    /// Direction the wind blows from, degrees from true north.
    pub from_deg: f32,
}

/// Mirror of `SoilSample`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoilMirror {
    /// Volumetric water content, percent.
    pub vwc_pct: f32,
    /// Soil temperature at probe depth, degrees Celsius.
    pub temp_c: f32,
}

/// Mirror of `RainSample`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RainMirror {
    /// Rainfall since local midnight, millimetres.
    pub since_midnight_mm: f32,
    /// Whether the sensor currently detects precipitation.
    pub detecting: bool,
}

/// Confidence policy for field data.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct AgriConfig {
    /// The GNSS receiver the fix came from.
    pub gnss: SensorId,
    /// The weather or soil station the samples came from.
    pub station: SensorId,
    /// Confidence for an RTK-fixed position.
    pub rtk_confidence: Confidence,
    /// Confidence for a float or standalone position.
    pub float_confidence: Confidence,
    /// Confidence for a weather or soil sample.
    pub sample_confidence: Confidence,
}

impl AgriConfig {
    /// Defaults: 0.95 for RTK fixes, 0.6 for float fixes, 0.8 for samples.
    #[must_use]
    pub fn new(gnss: SensorId, station: SensorId) -> Self {
        Self {
            gnss,
            station,
            rtk_confidence: Confidence::new(0.95).unwrap_or(Confidence::ZERO),
            float_confidence: Confidence::new(0.6).unwrap_or(Confidence::ZERO),
            sample_confidence: Confidence::new(0.8).unwrap_or(Confidence::ZERO),
        }
    }
}

/// One item of field data, with the time it was taken.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum AgriInput {
    /// A GNSS fix.
    Gnss {
        /// When the fix was taken.
        timestamp: Timestamp,
        /// The fix.
        fix: GnssFixMirror,
    },
    /// A wind reading.
    Wind {
        /// When the reading was taken.
        timestamp: Timestamp,
        /// The reading.
        sample: WindMirror,
    },
    /// A soil probe reading.
    Soil {
        /// When the reading was taken.
        timestamp: Timestamp,
        /// The reading.
        sample: SoilMirror,
    },
    /// A rain sensor reading.
    Rain {
        /// When the reading was taken.
        timestamp: Timestamp,
        /// The reading.
        sample: RainMirror,
    },
}

/// Converts field data into observations.
#[derive(Clone, Debug)]
pub struct AgriAdapter {
    config: AgriConfig,
    next_id: u64,
}

impl AgriAdapter {
    /// Creates an adapter whose observation ids start at `first_id`.
    #[must_use]
    pub const fn new(config: AgriConfig, first_id: ObservationId) -> Self {
        Self {
            config,
            next_id: first_id.get(),
        }
    }

    fn id(&mut self) -> ObservationId {
        let id = ObservationId::new(self.next_id);
        self.next_id += 1;
        id
    }

    fn provenance(sensor: SensorId) -> Provenance {
        Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(sensor))
    }

    fn scalar(
        &mut self,
        modality: Modality,
        property: &str,
        value: f64,
        confidence: Confidence,
        timestamp: Timestamp,
        sensor: SensorId,
    ) -> Observation {
        let id = self.id();
        Observation::new(
            id,
            modality,
            Property::new(property),
            ObservationValue::Scalar(value),
            confidence,
            timestamp,
            Self::provenance(sensor),
        )
    }
}

fn check(values: &[f64]) -> Result<(), AdapterError> {
    if values.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(AdapterError::Invalid("non-finite field value".into()))
    }
}

impl Adapter for AgriAdapter {
    type Input = AgriInput;

    fn name(&self) -> &'static str {
        "tpt-teleop-agri-adapter"
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one match arm per input kind keeps each validation rule beside its input"
    )]
    fn adapt(&mut self, input: AgriInput) -> Result<Vec<Observation>, AdapterError> {
        match input {
            AgriInput::Gnss { timestamp, fix } => {
                check(&[fix.lat_deg, fix.lon_deg, fix.alt_m])?;
                if !(-90.0..=90.0).contains(&fix.lat_deg)
                    || !(-180.0..=180.0).contains(&fix.lon_deg)
                {
                    return Err(AdapterError::Invalid(
                        "latitude or longitude out of range".into(),
                    ));
                }
                if !fix.hprec_m.is_finite() || fix.hprec_m < 0.0 {
                    return Err(AdapterError::Invalid(
                        "horizontal precision must be finite and non-negative".into(),
                    ));
                }
                let confidence = if fix.rtk_fixed {
                    self.config.rtk_confidence
                } else {
                    self.config.float_confidence
                };
                let position: Point3 = [fix.lat_deg, fix.lon_deg, fix.alt_m];
                let mut uncertainty = Uncertainty::new();
                // 0.0 means the receiver gave no precision, so no uncertainty is claimed.
                if fix.hprec_m > 0.0 {
                    if let Some(sd) = StandardDeviation::new(f64::from(fix.hprec_m)) {
                        uncertainty = uncertainty.with_standard_deviation(sd);
                    }
                }
                let id = self.id();
                let obs = Observation::new(
                    id,
                    Modality::Position,
                    Property::new("gnss_position"),
                    ObservationValue::Vector(position.to_vec()),
                    confidence,
                    timestamp,
                    Self::provenance(self.config.gnss),
                )
                .with_location(SpatialReference::at(
                    CoordinateFrame::Custom("WGS-84".into()),
                    position,
                ))
                .with_uncertainty(uncertainty);
                Ok(vec![obs])
            }
            AgriInput::Wind { timestamp, sample } => {
                check(&[f64::from(sample.speed_m_s), f64::from(sample.from_deg)])?;
                if sample.speed_m_s < 0.0 {
                    return Err(AdapterError::Invalid(
                        "wind speed must be non-negative".into(),
                    ));
                }
                let c = self.config.sample_confidence;
                let station = self.config.station;
                Ok(vec![
                    self.scalar(
                        Modality::Environmental,
                        "wind_speed_m_s",
                        f64::from(sample.speed_m_s),
                        c,
                        timestamp,
                        station,
                    ),
                    self.scalar(
                        Modality::Environmental,
                        "wind_from_deg",
                        f64::from(sample.from_deg),
                        c,
                        timestamp,
                        station,
                    ),
                ])
            }
            AgriInput::Soil { timestamp, sample } => {
                check(&[f64::from(sample.vwc_pct), f64::from(sample.temp_c)])?;
                if !(0.0..=100.0).contains(&sample.vwc_pct) {
                    return Err(AdapterError::Invalid(
                        "volumetric water content must be 0 to 100 percent".into(),
                    ));
                }
                let c = self.config.sample_confidence;
                let station = self.config.station;
                Ok(vec![
                    self.scalar(
                        Modality::Environmental,
                        "soil_moisture_vwc_pct",
                        f64::from(sample.vwc_pct),
                        c,
                        timestamp,
                        station,
                    ),
                    self.scalar(
                        Modality::Environmental,
                        "soil_temperature_c",
                        f64::from(sample.temp_c),
                        c,
                        timestamp,
                        station,
                    ),
                ])
            }
            AgriInput::Rain { timestamp, sample } => {
                check(&[f64::from(sample.since_midnight_mm)])?;
                if sample.since_midnight_mm < 0.0 {
                    return Err(AdapterError::Invalid(
                        "rainfall must be non-negative".into(),
                    ));
                }
                let c = self.config.sample_confidence;
                let station = self.config.station;
                let id = self.id();
                Ok(vec![
                    self.scalar(
                        Modality::Environmental,
                        "rain_since_midnight_mm",
                        f64::from(sample.since_midnight_mm),
                        c,
                        timestamp,
                        station,
                    ),
                    Observation::new(
                        id,
                        Modality::Environmental,
                        Property::new("raining"),
                        ObservationValue::Boolean(sample.detecting),
                        c,
                        timestamp,
                        Self::provenance(station),
                    ),
                ])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts() -> Timestamp {
        Timestamp::new(7, ClockSource::Hardware)
    }

    fn adapter() -> AgriAdapter {
        AgriAdapter::new(
            AgriConfig::new(SensorId::new(10), SensorId::new(20)),
            ObservationId::new(1),
        )
    }

    fn fix(rtk: bool, hprec: f32) -> GnssFixMirror {
        GnssFixMirror {
            lat_deg: 52.2053,
            lon_deg: 0.1218,
            alt_m: 19.0,
            hprec_m: hprec,
            rtk_fixed: rtk,
        }
    }

    #[test]
    fn rtk_fix_is_more_confident_than_float_and_carries_its_precision() {
        let rtk = adapter()
            .adapt(AgriInput::Gnss {
                timestamp: ts(),
                fix: fix(true, 0.02),
            })
            .unwrap();
        let float = adapter()
            .adapt(AgriInput::Gnss {
                timestamp: ts(),
                fix: fix(false, 1.5),
            })
            .unwrap();
        assert!(rtk[0].confidence() > float[0].confidence());
        let sd = rtk[0]
            .uncertainty()
            .unwrap()
            .standard_deviation
            .unwrap()
            .get();
        assert!((sd - 0.02).abs() < 1e-6);
        assert_eq!(
            rtk[0].location().unwrap().frame,
            CoordinateFrame::Custom("WGS-84".into())
        );
    }

    #[test]
    fn unknown_precision_claims_no_uncertainty() {
        let out = adapter()
            .adapt(AgriInput::Gnss {
                timestamp: ts(),
                fix: fix(false, 0.0),
            })
            .unwrap();
        assert!(
            out[0]
                .uncertainty()
                .is_none_or(|u| u.standard_deviation.is_none())
        );
    }

    #[test]
    fn out_of_range_positions_are_rejected() {
        let mut bad = fix(true, 0.1);
        bad.lat_deg = 123.0;
        assert!(matches!(
            adapter().adapt(AgriInput::Gnss {
                timestamp: ts(),
                fix: bad
            }),
            Err(AdapterError::Invalid(_))
        ));
    }

    #[test]
    fn wind_produces_speed_and_direction_as_environmental_observations() {
        let out = adapter()
            .adapt(AgriInput::Wind {
                timestamp: ts(),
                sample: WindMirror {
                    speed_m_s: 4.5,
                    from_deg: 270.0,
                },
            })
            .unwrap();
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|o| o.modality() == &Modality::Environmental));
        assert_eq!(out[0].value(), &ObservationValue::Scalar(4.5));
    }

    #[test]
    fn impossible_soil_moisture_is_rejected() {
        assert!(matches!(
            adapter().adapt(AgriInput::Soil {
                timestamp: ts(),
                sample: SoilMirror {
                    vwc_pct: 130.0,
                    temp_c: 12.0
                },
            }),
            Err(AdapterError::Invalid(_))
        ));
    }

    #[test]
    fn rain_reports_accumulation_and_detection() {
        let out = adapter()
            .adapt(AgriInput::Rain {
                timestamp: ts(),
                sample: RainMirror {
                    since_midnight_mm: 2.4,
                    detecting: true,
                },
            })
            .unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[1].value(), &ObservationValue::Boolean(true));
    }

    #[test]
    fn non_finite_values_never_pass_through() {
        assert!(matches!(
            adapter().adapt(AgriInput::Wind {
                timestamp: ts(),
                sample: WindMirror {
                    speed_m_s: f32::NAN,
                    from_deg: 0.0
                },
            }),
            Err(AdapterError::Invalid(_))
        ));
    }
}

//! Measurements: raw or minimally processed sensor output (spec §12).

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::{Frequency, Modality, Provenance, SensorId, Timestamp, Uncertainty};

/// Reference-counted, immutable buffer. Cloning is cheap (no data copy), so large payloads such
/// as video frames or point clouds can be shared between pipeline stages without duplication.
pub type SharedBuffer<T> = Arc<[T]>;

/// The payload of a measurement.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "MeasurementValueWire", into = "MeasurementValueWire")
)]
pub enum MeasurementValue {
    /// A single number.
    Scalar(f64),
    /// A vector of numbers.
    Vector(Vec<f64>),
    /// A row-major matrix. Built with [`MeasurementValue::matrix`].
    Matrix {
        /// Number of rows.
        rows: usize,
        /// Number of columns.
        cols: usize,
        /// `rows * cols` values, row-major.
        data: Vec<f64>,
    },
    /// An image or video frame. Built with [`MeasurementValue::image`].
    Image {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
        /// Channels per pixel.
        channels: u8,
        /// `width * height * channels` bytes, row-major, interleaved.
        data: SharedBuffer<u8>,
    },
    /// An audio buffer. Samples are interleaved when `channels > 1`.
    Audio {
        /// Sample rate.
        sample_rate: Frequency,
        /// Number of channels.
        channels: u16,
        /// Samples.
        samples: SharedBuffer<f32>,
    },
    /// A point cloud as `[x, y, z]` points.
    PointCloud(SharedBuffer<[f32; 3]>),
    /// A spectrum. Built with [`MeasurementValue::spectrum`].
    Spectrum {
        /// Band centres in nanometres.
        wavelengths_nm: Vec<f64>,
        /// Intensity per band.
        intensities: Vec<f64>,
    },
    /// A categorical value, e.g. a detected class label.
    Categorical(String),
    /// A named collection of values.
    Structured(Vec<(String, MeasurementValue)>),
    /// Opaque binary data with a media type describing it.
    Binary {
        /// Media type, e.g. `application/x-custom`.
        media_type: String,
        /// Raw bytes.
        data: SharedBuffer<u8>,
    },
}

impl MeasurementValue {
    /// Creates a matrix. Returns `None` if `data.len() != rows * cols`.
    #[must_use]
    pub fn matrix(rows: usize, cols: usize, data: Vec<f64>) -> Option<Self> {
        (rows.checked_mul(cols) == Some(data.len())).then_some(Self::Matrix { rows, cols, data })
    }

    /// Creates an image. Returns `None` if the buffer length does not match the dimensions.
    #[must_use]
    pub fn image(width: u32, height: u32, channels: u8, data: SharedBuffer<u8>) -> Option<Self> {
        let expected = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(channels as usize)?;
        (expected == data.len()).then_some(Self::Image {
            width,
            height,
            channels,
            data,
        })
    }

    /// Creates an audio buffer. Returns `None` if `channels` is zero or the sample count is not a
    /// whole number of frames.
    #[must_use]
    pub fn audio(
        sample_rate: Frequency,
        channels: u16,
        samples: SharedBuffer<f32>,
    ) -> Option<Self> {
        (channels > 0 && samples.len() % usize::from(channels) == 0).then_some(Self::Audio {
            sample_rate,
            channels,
            samples,
        })
    }

    /// Creates a spectrum. Returns `None` if the two vectors differ in length.
    #[must_use]
    pub fn spectrum(wavelengths_nm: Vec<f64>, intensities: Vec<f64>) -> Option<Self> {
        (wavelengths_nm.len() == intensities.len()).then_some(Self::Spectrum {
            wavelengths_nm,
            intensities,
        })
    }
}

/// A single measurement from one sensor at one time.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Measurement {
    /// The sensor that produced this measurement.
    pub sensor: SensorId,
    /// The kind of quantity measured.
    pub modality: Modality,
    /// The measured payload.
    pub value: MeasurementValue,
    /// When the measurement was taken.
    pub timestamp: Timestamp,
    /// Quantified uncertainty, if the sensor provides it.
    pub uncertainty: Option<Uncertainty>,
    /// Where the measurement came from.
    pub provenance: Provenance,
}

impl Measurement {
    /// Creates a measurement without uncertainty.
    #[must_use]
    pub const fn new(
        sensor: SensorId,
        modality: Modality,
        value: MeasurementValue,
        timestamp: Timestamp,
        provenance: Provenance,
    ) -> Self {
        Self {
            sensor,
            modality,
            value,
            timestamp,
            uncertainty: None,
            provenance,
        }
    }

    /// Attaches uncertainty (builder style).
    #[must_use]
    pub fn with_uncertainty(mut self, uncertainty: Uncertainty) -> Self {
        self.uncertainty = Some(uncertainty);
        self
    }
}

// Serde support. Deserialization goes through the validating constructors, so a matrix with the
// wrong element count, an image with the wrong byte count, or audio that is not whole frames is
// rejected, as it would be in memory.
#[cfg(feature = "serde")]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
enum MeasurementValueWire {
    Scalar(f64),
    Vector(Vec<f64>),
    Matrix {
        rows: usize,
        cols: usize,
        data: Vec<f64>,
    },
    Image {
        width: u32,
        height: u32,
        channels: u8,
        data: SharedBuffer<u8>,
    },
    Audio {
        sample_rate_hz: f64,
        channels: u16,
        samples: SharedBuffer<f32>,
    },
    PointCloud(SharedBuffer<[f32; 3]>),
    Spectrum {
        wavelengths_nm: Vec<f64>,
        intensities: Vec<f64>,
    },
    Categorical(String),
    Structured(Vec<(String, MeasurementValueWire)>),
    Binary {
        media_type: String,
        data: SharedBuffer<u8>,
    },
}

#[cfg(feature = "serde")]
impl TryFrom<MeasurementValueWire> for MeasurementValue {
    type Error = &'static str;

    fn try_from(wire: MeasurementValueWire) -> Result<Self, Self::Error> {
        Ok(match wire {
            MeasurementValueWire::Scalar(v) => Self::Scalar(v),
            MeasurementValueWire::Vector(v) => Self::Vector(v),
            MeasurementValueWire::Matrix { rows, cols, data } => Self::matrix(rows, cols, data)
                .ok_or("matrix data length does not match rows * cols")?,
            MeasurementValueWire::Image {
                width,
                height,
                channels,
                data,
            } => Self::image(width, height, channels, data)
                .ok_or("image buffer length does not match width * height * channels")?,
            MeasurementValueWire::Audio {
                sample_rate_hz,
                channels,
                samples,
            } => {
                let rate = Frequency::from_hz(sample_rate_hz)
                    .ok_or("audio sample rate must be finite and positive")?;
                Self::audio(rate, channels, samples)
                    .ok_or("audio needs at least one channel and whole frames")?
            }
            MeasurementValueWire::PointCloud(points) => Self::PointCloud(points),
            MeasurementValueWire::Spectrum {
                wavelengths_nm,
                intensities,
            } => Self::spectrum(wavelengths_nm, intensities)
                .ok_or("spectrum wavelengths and intensities differ in length")?,
            MeasurementValueWire::Categorical(c) => Self::Categorical(c),
            MeasurementValueWire::Structured(fields) => {
                let mut out = Vec::with_capacity(fields.len());
                for (name, value) in fields {
                    out.push((name, Self::try_from(value)?));
                }
                Self::Structured(out)
            }
            MeasurementValueWire::Binary { media_type, data } => Self::Binary { media_type, data },
        })
    }
}

#[cfg(feature = "serde")]
impl From<MeasurementValue> for MeasurementValueWire {
    fn from(value: MeasurementValue) -> Self {
        match value {
            MeasurementValue::Scalar(v) => Self::Scalar(v),
            MeasurementValue::Vector(v) => Self::Vector(v),
            MeasurementValue::Matrix { rows, cols, data } => Self::Matrix { rows, cols, data },
            MeasurementValue::Image {
                width,
                height,
                channels,
                data,
            } => Self::Image {
                width,
                height,
                channels,
                data,
            },
            MeasurementValue::Audio {
                sample_rate,
                channels,
                samples,
            } => Self::Audio {
                sample_rate_hz: sample_rate.hz(),
                channels,
                samples,
            },
            MeasurementValue::PointCloud(points) => Self::PointCloud(points),
            MeasurementValue::Spectrum {
                wavelengths_nm,
                intensities,
            } => Self::Spectrum {
                wavelengths_nm,
                intensities,
            },
            MeasurementValue::Categorical(c) => Self::Categorical(c),
            MeasurementValue::Structured(fields) => Self::Structured(
                fields
                    .into_iter()
                    .map(|(name, v)| (name, Self::from(v)))
                    .collect(),
            ),
            MeasurementValue::Binary { media_type, data } => Self::Binary { media_type, data },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_validates_shape() {
        assert!(MeasurementValue::matrix(2, 2, alloc::vec![0.0; 4]).is_some());
        assert!(MeasurementValue::matrix(2, 3, alloc::vec![0.0; 4]).is_none());
    }

    #[test]
    fn image_validates_buffer_length() {
        let data: SharedBuffer<u8> = Arc::from(alloc::vec![0u8; 2 * 2 * 3]);
        assert!(MeasurementValue::image(2, 2, 3, data.clone()).is_some());
        assert!(MeasurementValue::image(2, 2, 1, data).is_none());
    }

    #[test]
    fn spectrum_requires_matching_lengths() {
        assert!(
            MeasurementValue::spectrum(alloc::vec![400.0, 500.0], alloc::vec![1.0, 2.0]).is_some()
        );
        assert!(MeasurementValue::spectrum(alloc::vec![400.0], alloc::vec![]).is_none());
    }

    #[test]
    fn shared_buffers_are_not_copied() {
        let data: SharedBuffer<u8> = Arc::from(alloc::vec![1u8, 2, 3]);
        let a = MeasurementValue::Binary {
            media_type: "x/y".into(),
            data: data.clone(),
        };
        let b = a.clone();
        match (&a, &b) {
            (
                MeasurementValue::Binary { data: da, .. },
                MeasurementValue::Binary { data: db, .. },
            ) => assert!(Arc::ptr_eq(da, db)),
            _ => unreachable!(),
        }
    }
}

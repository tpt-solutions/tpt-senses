//! Measurements: raw or minimally processed sensor output (spec §12).

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::{Frequency, Modality, Provenance, SensorId, Timestamp, Uncertainty};

/// Reference-counted, immutable buffer. Cloning is cheap (no data copy), so large payloads such
/// as video frames or point clouds can be shared between pipeline stages without duplication.
pub type SharedBuffer<T> = Arc<[T]>;

/// A row-major matrix. Its element count is checked at construction, so `rows * cols` always
/// equals the number of values. Build with [`MeasurementValue::matrix`].
#[derive(Clone, Debug, PartialEq)]
pub struct Matrix {
    rows: usize,
    cols: usize,
    data: Vec<f64>,
}

impl Matrix {
    /// Number of rows.
    #[must_use]
    pub const fn rows(&self) -> usize {
        self.rows
    }

    /// Number of columns.
    #[must_use]
    pub const fn cols(&self) -> usize {
        self.cols
    }

    /// The values, row-major.
    #[must_use]
    pub fn data(&self) -> &[f64] {
        &self.data
    }
}

/// An image or video frame. Its byte count is checked at construction. Build with
/// [`MeasurementValue::image`].
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    width: u32,
    height: u32,
    channels: u8,
    data: SharedBuffer<u8>,
}

impl Image {
    /// Width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Channels per pixel.
    #[must_use]
    pub const fn channels(&self) -> u8 {
        self.channels
    }

    /// The bytes, row-major and interleaved.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

/// An audio buffer whose sample count is a whole number of frames. Build with
/// [`MeasurementValue::audio`].
#[derive(Clone, Debug, PartialEq)]
pub struct Audio {
    sample_rate: Frequency,
    channels: u16,
    samples: SharedBuffer<f32>,
}

impl Audio {
    /// Sample rate.
    #[must_use]
    pub const fn sample_rate(&self) -> Frequency {
        self.sample_rate
    }

    /// Number of channels (at least one).
    #[must_use]
    pub const fn channels(&self) -> u16 {
        self.channels
    }

    /// The samples, interleaved when there is more than one channel.
    #[must_use]
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }
}

/// A spectrum with one intensity per band. Build with [`MeasurementValue::spectrum`].
#[derive(Clone, Debug, PartialEq)]
pub struct Spectrum {
    wavelengths_nm: Vec<f64>,
    intensities: Vec<f64>,
}

impl Spectrum {
    /// Band centres in nanometres.
    #[must_use]
    pub fn wavelengths_nm(&self) -> &[f64] {
        &self.wavelengths_nm
    }

    /// Intensity per band.
    #[must_use]
    pub fn intensities(&self) -> &[f64] {
        &self.intensities
    }
}

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
    Matrix(Matrix),
    /// An image or video frame. Built with [`MeasurementValue::image`].
    Image(Image),
    /// An audio buffer. Built with [`MeasurementValue::audio`].
    Audio(Audio),
    /// A point cloud as `[x, y, z]` points.
    PointCloud(SharedBuffer<[f32; 3]>),
    /// A spectrum. Built with [`MeasurementValue::spectrum`].
    Spectrum(Spectrum),
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
        (rows.checked_mul(cols) == Some(data.len())).then_some(Self::Matrix(Matrix {
            rows,
            cols,
            data,
        }))
    }

    /// Creates an image. Returns `None` if the buffer length does not match the dimensions.
    #[must_use]
    pub fn image(width: u32, height: u32, channels: u8, data: SharedBuffer<u8>) -> Option<Self> {
        let expected = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(channels as usize)?;
        (expected == data.len()).then_some(Self::Image(Image {
            width,
            height,
            channels,
            data,
        }))
    }

    /// Creates an audio buffer. Returns `None` if `channels` is zero or the sample count is not a
    /// whole number of frames.
    #[must_use]
    pub fn audio(
        sample_rate: Frequency,
        channels: u16,
        samples: SharedBuffer<f32>,
    ) -> Option<Self> {
        (channels > 0 && samples.len() % usize::from(channels) == 0).then_some(Self::Audio(Audio {
            sample_rate,
            channels,
            samples,
        }))
    }

    /// Creates a spectrum. Returns `None` if the two vectors differ in length.
    #[must_use]
    pub fn spectrum(wavelengths_nm: Vec<f64>, intensities: Vec<f64>) -> Option<Self> {
        (wavelengths_nm.len() == intensities.len()).then_some(Self::Spectrum(Spectrum {
            wavelengths_nm,
            intensities,
        }))
    }
}

/// A single measurement from one sensor at one time.
///
/// # Example
///
/// ```
/// use tpt_senses::prelude::*;
/// use tpt_senses::StandardDeviation;
///
/// let reading = Measurement::new(
///     SensorId::new(3),
///     Modality::Thermal,
///     MeasurementValue::Scalar(21.5),
///     Timestamp::new(2_000_000, ClockSource::Monotonic),
///     Provenance::new(Origin::Real),
/// )
/// .with_uncertainty(
///     Uncertainty::new().with_standard_deviation(StandardDeviation::new(0.2).unwrap()),
/// );
///
/// assert_eq!(reading.sensor, SensorId::new(3));
/// assert!(reading.uncertainty.is_some());
/// ```
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
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
            MeasurementValue::Matrix(m) => Self::Matrix {
                rows: m.rows,
                cols: m.cols,
                data: m.data,
            },
            MeasurementValue::Image(i) => Self::Image {
                width: i.width,
                height: i.height,
                channels: i.channels,
                data: i.data,
            },
            MeasurementValue::Audio(a) => Self::Audio {
                sample_rate_hz: a.sample_rate.hz(),
                channels: a.channels,
                samples: a.samples,
            },
            MeasurementValue::PointCloud(points) => Self::PointCloud(points),
            MeasurementValue::Spectrum(sp) => Self::Spectrum {
                wavelengths_nm: sp.wavelengths_nm,
                intensities: sp.intensities,
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
    fn accessors_report_the_validated_shape() {
        let MeasurementValue::Matrix(m) =
            MeasurementValue::matrix(2, 3, alloc::vec![0.0; 6]).unwrap()
        else {
            panic!("expected a matrix")
        };
        assert_eq!((m.rows(), m.cols(), m.data().len()), (2, 3, 6));
        let MeasurementValue::Spectrum(sp) =
            MeasurementValue::spectrum(alloc::vec![1.0], alloc::vec![2.0]).unwrap()
        else {
            panic!("expected a spectrum")
        };
        assert_eq!(sp.intensities(), &[2.0]);
    }

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

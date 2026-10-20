//! Exact finite signal processing primitive checks.
#![allow(clippy::struct_excessive_bools)]
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FiniteSampleSequence {
    pub sample_count: usize,
    pub max_samples: usize,
    pub sample_width_bits: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FirFilterEnvelope {
    pub tap_count: usize,
    pub max_taps: usize,
    pub input_len: usize,
    pub output_len: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ConvolutionWindow {
    pub window_start: usize,
    pub window_end: usize,
    pub input_len: usize,
    pub kernel_len: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DecimationAdmission {
    pub factor: usize,
    pub input_len: usize,
    pub output_len: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct InterpolationAdmission {
    pub factor: usize,
    pub input_len: usize,
    pub output_len: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PeakCandidate {
    pub index: usize,
    pub sample_count: usize,
    pub left: i64,
    pub value: i64,
    pub right: i64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SpectralBinMetadata {
    pub bin_index: usize,
    pub bin_count: usize,
    pub max_bins: usize,
    pub frequency_num: usize,
    pub frequency_den: usize,
}

// r[depends signal.finite-sample-sequence]
pub open spec fn finite_sample_sequence_spec(e: FiniteSampleSequence) -> bool {
    e.sample_count <= e.max_samples && e.sample_width_bits <= 64
}

// r[impl signal.finite-sample-sequence]
pub fn finite_sample_sequence_valid(e: FiniteSampleSequence) -> (result: bool)
    ensures result == finite_sample_sequence_spec(e)
{
    e.sample_count <= e.max_samples && e.sample_width_bits <= 64
}

// r[verify signal.finite-sample-sequence]
pub proof fn valid_finite_sample_sequence_is_admitted(e: FiniteSampleSequence)
    requires finite_sample_sequence_spec(e)
    ensures finite_sample_sequence_spec(e)
{}

// r[depends signal.fir-filter-envelope]
pub open spec fn fir_filter_envelope_spec(e: FirFilterEnvelope) -> bool {
    e.tap_count > 0 && e.tap_count <= e.max_taps && e.output_len <= e.input_len
}

// r[impl signal.fir-filter-envelope]
pub fn fir_filter_envelope_valid(e: FirFilterEnvelope) -> (result: bool)
    ensures result == fir_filter_envelope_spec(e)
{
    e.tap_count > 0 && e.tap_count <= e.max_taps && e.output_len <= e.input_len
}

// r[verify signal.fir-filter-envelope]
pub proof fn valid_fir_filter_envelope_is_admitted(e: FirFilterEnvelope)
    requires fir_filter_envelope_spec(e)
    ensures fir_filter_envelope_spec(e)
{}

// r[depends signal.convolution-window]
pub open spec fn convolution_window_spec(e: ConvolutionWindow) -> bool {
    e.window_start <= e.window_end && e.window_end <= e.input_len && e.kernel_len > 0
}

// r[impl signal.convolution-window]
pub fn convolution_window_valid(e: ConvolutionWindow) -> (result: bool)
    ensures result == convolution_window_spec(e)
{
    e.window_start <= e.window_end && e.window_end <= e.input_len && e.kernel_len > 0
}

// r[verify signal.convolution-window]
pub proof fn valid_convolution_window_is_admitted(e: ConvolutionWindow)
    requires convolution_window_spec(e)
    ensures convolution_window_spec(e)
{}

// r[depends signal.decimation-admission]
pub open spec fn decimation_admission_spec(e: DecimationAdmission) -> bool {
    e.factor > 0 && e.output_len <= e.input_len / e.factor
}

// r[impl signal.decimation-admission]
pub fn decimation_admission_valid(e: DecimationAdmission) -> (result: bool)
    ensures result == decimation_admission_spec(e)
{
    e.factor > 0 && e.output_len <= e.input_len / e.factor
}

// r[verify signal.decimation-admission]
pub proof fn valid_decimation_admission_is_admitted(e: DecimationAdmission)
    requires decimation_admission_spec(e)
    ensures decimation_admission_spec(e)
{}

// r[depends signal.interpolation-admission]
pub open spec fn interpolation_admission_spec(e: InterpolationAdmission) -> bool {
    e.factor > 0 && e.input_len <= e.output_len
}

// r[impl signal.interpolation-admission]
pub fn interpolation_admission_valid(e: InterpolationAdmission) -> (result: bool)
    ensures result == interpolation_admission_spec(e)
{
    e.factor > 0 && e.input_len <= e.output_len
}

// r[verify signal.interpolation-admission]
pub proof fn valid_interpolation_admission_is_admitted(e: InterpolationAdmission)
    requires interpolation_admission_spec(e)
    ensures interpolation_admission_spec(e)
{}

// r[depends signal.peak-candidate]
pub open spec fn peak_candidate_spec(e: PeakCandidate) -> bool {
    e.sample_count > 1 && e.index > 0 && e.index < e.sample_count - 1 && e.left <= e.value && e.right <= e.value
}

// r[impl signal.peak-candidate]
pub fn peak_candidate_valid(e: PeakCandidate) -> (result: bool)
    ensures result == peak_candidate_spec(e)
{
    e.sample_count > 1 && e.index > 0 && e.index < e.sample_count - 1 && e.left <= e.value && e.right <= e.value
}

// r[verify signal.peak-candidate]
pub proof fn valid_peak_candidate_is_admitted(e: PeakCandidate)
    requires peak_candidate_spec(e)
    ensures peak_candidate_spec(e)
{}

// r[depends signal.spectral-bin-metadata]
pub open spec fn spectral_bin_metadata_spec(e: SpectralBinMetadata) -> bool {
    e.bin_index < e.bin_count && e.bin_count <= e.max_bins && e.frequency_den > 0 && e.frequency_num <= e.frequency_den
}

// r[impl signal.spectral-bin-metadata]
pub fn spectral_bin_metadata_valid(e: SpectralBinMetadata) -> (result: bool)
    ensures result == spectral_bin_metadata_spec(e)
{
    e.bin_index < e.bin_count && e.bin_count <= e.max_bins && e.frequency_den > 0 && e.frequency_num <= e.frequency_den
}

// r[verify signal.spectral-bin-metadata]
pub proof fn valid_spectral_bin_metadata_is_admitted(e: SpectralBinMetadata)
    requires spectral_bin_metadata_spec(e)
    ensures spectral_bin_metadata_spec(e)
{}

} // verus!

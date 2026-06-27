use renderer_api::{DEFAULT_MSAA_SAMPLES, RenderLimitProfile, RenderSettings};

#[test]
fn renderer_defaults_to_low_spec_limit_profile() {
    let settings = RenderSettings::default();

    assert_eq!(settings.limit_profile, RenderLimitProfile::LowSpec);
}

#[test]
fn renderer_defaults_to_four_sample_msaa() {
    let settings = RenderSettings::default();

    assert_eq!(DEFAULT_MSAA_SAMPLES, 4);
    assert_eq!(settings.msaa_samples, DEFAULT_MSAA_SAMPLES);
}

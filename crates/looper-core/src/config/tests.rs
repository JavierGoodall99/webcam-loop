use super::*;

#[test]
fn the_defaults_at_30_fps_are_in_frames() {
    let c = Config::for_fps(30).unwrap();
    assert_eq!(c.fps, 30);
    assert_eq!(c.crossfade_frames, 12); // 400 ms
    assert_eq!(c.min_record_frames, 23); // 750 ms, rounded to nearest
    assert_eq!(c.max_record_frames, 600); // 20 s
    assert_eq!(c.max_record_bytes, 512 * 1024 * 1024);
}

#[test]
fn the_defaults_scale_with_the_frame_rate() {
    let c = Config::for_fps(60).unwrap();
    assert_eq!(
        (c.crossfade_frames, c.min_record_frames, c.max_record_frames),
        (24, 45, 1200)
    );
}

#[test]
fn a_frame_rate_of_zero_or_above_the_limit_is_rejected() {
    assert_eq!(Config::for_fps(0), Err(ConfigError::ZeroFps));
    assert_eq!(Config::for_fps(241), Err(ConfigError::FpsTooHigh));
    assert!(Config::for_fps(240).is_ok());
    assert!(Config::for_fps(1).is_ok());
}

#[test]
fn even_a_one_fps_default_has_a_valid_minimum_hold() {
    let c = Config::for_fps(1).unwrap();
    assert!(c.min_record_frames >= 1 && c.min_record_frames <= c.max_record_frames);
}

fn valid() -> Config {
    Config {
        fps: 30,
        crossfade_frames: 5,
        min_record_frames: 10,
        max_record_frames: 100,
        max_record_bytes: 1000,
    }
}

#[test]
fn validate_accepts_a_sane_config_including_no_crossfade() {
    assert_eq!(valid().validate(), Ok(()));
    assert_eq!(
        Config {
            crossfade_frames: 0,
            ..valid()
        }
        .validate(),
        Ok(())
    );
    assert_eq!(
        Config {
            min_record_frames: 100,
            ..valid()
        }
        .validate(),
        Ok(())
    ); // min == max is allowed
}

#[test]
fn validate_rejects_each_kind_of_nonsense() {
    assert_eq!(
        Config { fps: 0, ..valid() }.validate(),
        Err(ConfigError::ZeroFps)
    );
    assert_eq!(
        Config {
            min_record_frames: 0,
            ..valid()
        }
        .validate(),
        Err(ConfigError::ZeroMinRecord)
    );
    assert_eq!(
        Config {
            min_record_frames: 101,
            ..valid()
        }
        .validate(),
        Err(ConfigError::MinAboveMax)
    );
    assert_eq!(
        Config {
            max_record_bytes: 0,
            ..valid()
        }
        .validate(),
        Err(ConfigError::ZeroMaxBytes)
    );
}

//! Command line parsing. Pure functions so that they can be unit tested.

use std::fmt;

pub const USAGE: &str = "usage: game [--smoke-test <frames>]";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// Run the real application for this many frames, then exit (code 0 on success).
    pub smoke_test_frames: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CliError {
    MissingFrameCount,
    InvalidFrameCount(String),
    DuplicateSmokeTest,
    UnknownArgument(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::MissingFrameCount => write!(f, "--smoke-test requires a frame count"),
            CliError::InvalidFrameCount(value) => write!(
                f,
                "invalid frame count {value:?} for --smoke-test: expected an integer from 1 to {}",
                u32::MAX
            ),
            CliError::DuplicateSmokeTest => write!(f, "--smoke-test given more than once"),
            CliError::UnknownArgument(arg) => write!(f, "unknown argument {arg:?}"),
        }
    }
}

/// Parses the arguments that follow the program name.
pub fn parse<I>(args: I) -> Result<Options, CliError>
where
    I: IntoIterator<Item = String>,
{
    let mut options = Options::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--smoke-test" => {
                if options.smoke_test_frames.is_some() {
                    return Err(CliError::DuplicateSmokeTest);
                }
                let value = args.next().ok_or(CliError::MissingFrameCount)?;
                let frames = value
                    .parse::<u32>()
                    .ok()
                    .filter(|&frames| frames > 0)
                    .ok_or(CliError::InvalidFrameCount(value))?;
                options.smoke_test_frames = Some(frames);
            }
            _ => return Err(CliError::UnknownArgument(arg)),
        }
    }
    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_strs(args: &[&str]) -> Result<Options, CliError> {
        parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn no_arguments_runs_normally() {
        assert_eq!(parse_strs(&[]), Ok(Options::default()));
    }

    #[test]
    fn smoke_test_with_frame_count() {
        assert_eq!(
            parse_strs(&["--smoke-test", "600"]),
            Ok(Options {
                smoke_test_frames: Some(600)
            })
        );
    }

    #[test]
    fn smoke_test_frame_count_boundaries() {
        assert_eq!(
            parse_strs(&["--smoke-test", "1"]).map(|o| o.smoke_test_frames),
            Ok(Some(1))
        );
        assert_eq!(
            parse_strs(&["--smoke-test", "4294967295"]).map(|o| o.smoke_test_frames),
            Ok(Some(u32::MAX))
        );
        for bad in ["0", "4294967296", "-1", "", "ten", "1.5", " 5"] {
            assert_eq!(
                parse_strs(&["--smoke-test", bad]),
                Err(CliError::InvalidFrameCount(bad.to_string())),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn smoke_test_without_frame_count_is_rejected() {
        assert_eq!(
            parse_strs(&["--smoke-test"]),
            Err(CliError::MissingFrameCount)
        );
    }

    #[test]
    fn repeated_smoke_test_is_rejected() {
        assert_eq!(
            parse_strs(&["--smoke-test", "1", "--smoke-test", "2"]),
            Err(CliError::DuplicateSmokeTest)
        );
    }

    #[test]
    fn unknown_arguments_are_rejected() {
        assert_eq!(
            parse_strs(&["--smoke"]),
            Err(CliError::UnknownArgument("--smoke".to_string()))
        );
        assert_eq!(
            parse_strs(&["--smoke-test", "5", "extra"]),
            Err(CliError::UnknownArgument("extra".to_string()))
        );
    }

    #[test]
    fn errors_have_descriptive_messages() {
        assert!(
            CliError::InvalidFrameCount("0".into())
                .to_string()
                .contains("\"0\"")
        );
        assert!(
            CliError::UnknownArgument("-x".into())
                .to_string()
                .contains("\"-x\"")
        );
    }
}

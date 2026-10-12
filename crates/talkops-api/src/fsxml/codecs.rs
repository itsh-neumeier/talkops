//! Codec profiles of devices in FreeSWITCH terms (ADR 0023).
//!
//! A device with a profile other than `standard` always uses the profile's
//! codecs on its own leg. FreeSWITCH transcodes to the codec of the other
//! party (trunk, browser, other phone).

use talkops_core::extensions::CodecProfile;

/// Media settings of a device leg.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegMedia {
    /// `absolute_codec_string` of the leg.
    pub codecs: &'static str,
    /// `jitterbuffer_msec` of the leg: initial and maximum length in ms.
    pub jitter_buffer: &'static str,
}

/// G.722 wideband audio; G.711 if the device does not accept G.722.
const HD_CODECS: &str = "G722,PCMA,PCMU";
/// iLBC (RFC 3951): every frame decodes on its own, so a lost packet does not
/// damage the next ones. 20 ms frames first: a lost packet removes less audio.
const ROBUST_CODECS: &str = "iLBC@20i,iLBC@30i,PCMA,PCMU";
/// Absorbs the delay variation of Wi-Fi without a long fixed delay.
const JITTER_BUFFER: &str = "60:200";

/// Media settings for a device leg; `None` for `standard`, where the codec
/// lists of the dialplan apply.
pub fn leg_media(profile: CodecProfile) -> Option<LegMedia> {
    match profile {
        CodecProfile::Standard => None,
        CodecProfile::Hd => Some(LegMedia {
            codecs: HD_CODECS,
            jitter_buffer: JITTER_BUFFER,
        }),
        CodecProfile::Robust => Some(LegMedia {
            codecs: ROBUST_CODECS,
            jitter_buffer: JITTER_BUFFER,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_map_to_codecs() {
        assert_eq!(leg_media(CodecProfile::Standard), None);
        assert_eq!(
            leg_media(CodecProfile::Hd).unwrap().codecs,
            "G722,PCMA,PCMU"
        );
        let robust = leg_media(CodecProfile::Robust).unwrap();
        assert!(robust.codecs.starts_with("iLBC@20i,iLBC@30i"));
        assert!(
            robust.codecs.ends_with("PCMA,PCMU"),
            "G.711 stays as fallback"
        );
    }
}

//! Stage91: configure long-running QEMU scene presentations without changing
//! production login gates or the historic twelve-frame acceptance tests.
//!
//! The DRM receiver already accepts a bounded 8..=64-frame stream. This
//! parser lets the Smithay bridge and *real* Wayland client agree on a larger
//! target; both retain twelve frames by default. Limits intentionally fit
//! the receiver's existing framebuffer and timeout protections.
pub const DEFAULT_QEMU_FRAMES:u64=12;
pub const MAX_QEMU_FRAMES:u64=64;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum FrameBudgetError { InvalidDefault, InvalidValue }

pub fn frame_budget(raw:Option<&str>,default:u64)->Result<u64,FrameBudgetError> {
    if !(DEFAULT_QEMU_FRAMES..=MAX_QEMU_FRAMES).contains(&default) {
        return Err(FrameBudgetError::InvalidDefault);
    }
    match raw {
        None=>Ok(default),
        Some(value)=>{
            // Do not silently ignore an invalid opt-in: the DRM worker must
            // agree with the client on its count, or both must fail closed.
            let parsed=value.parse::<u64>().map_err(|_|FrameBudgetError::InvalidValue)?;
            if !(DEFAULT_QEMU_FRAMES..=MAX_QEMU_FRAMES).contains(&parsed) {
                return Err(FrameBudgetError::InvalidValue);
            }
            Ok(parsed)
        }
    }
}


/// The external DRM/KMS receiver must reject invalid settings rather than
/// silently falling back to 12 and deadlocking against a different sender.
/// Historical (pre-Stage91) VM trials may request 8..=11 frames; GTK-only
/// captures may request *exactly* 1, 2 or 3 when explicitly selected.
pub fn receiver_frame_budget(
    raw: Option<&str>, gtk_expected: Option<u64>,
) -> Result<u64, FrameBudgetError> {
    let count = match raw {
        Some(value) => {
            if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                return Err(FrameBudgetError::InvalidValue);
            }
            value.parse::<u64>().map_err(|_| FrameBudgetError::InvalidValue)?
        }
        None if gtk_expected.is_some() => return Err(FrameBudgetError::InvalidValue),
        None => DEFAULT_QEMU_FRAMES,
    };
    let valid = if let Some(gtk) = gtk_expected {
        (1..=3).contains(&gtk) && count == gtk
    } else {
        (8..=MAX_QEMU_FRAMES).contains(&count)
    };
    if !valid { return Err(FrameBudgetError::InvalidValue); }
    Ok(count)
}

/// Direct GBM has two distinct finite acceptance contracts. The additional
/// frame in retained mode is compositor-only XDG destruction, not an extra
/// DMA-BUF import. Refuse missing, arbitrary or mismatched configuration.
pub fn direct_gpu_frame_budget_matches(multi:Option<&str>,frames:Option<&str>)->bool {
    matches!((multi,frames),(None,Some("12"))|(Some("YES_QEMU_VM"),Some("13")))
}

pub fn direct_gpu_move_budget_matches(multi:Option<&str>,motion:Option<&str>,frames:Option<&str>)->bool {
    match motion {None=>direct_gpu_frame_budget_matches(multi,frames),
        Some("YES_QEMU_VM")=>multi==Some("YES_QEMU_VM") && frames==Some("14"),
        Some("LIVE_QEMU_VM")=>multi==Some("YES_QEMU_VM") && frames==Some("15"),
        Some("VISIBILITY_QEMU_VM")=>multi==Some("YES_QEMU_VM") && frames==Some("17"),_=>false}
}

impl std::fmt::Display for FrameBudgetError {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {
        write!(f,"invalid QEMU continuous frame budget")
    }
}
impl std::error::Error for FrameBudgetError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_gpu_exact_mode_budget_rejects_cross_mode_and_invalid_counts() {
        assert!(direct_gpu_frame_budget_matches(None,Some("12")));
        assert!(direct_gpu_frame_budget_matches(Some("YES_QEMU_VM"),Some("13")));
        for mode in [None,Some("YES_QEMU_VM"),Some("NO"),Some("")] {
            for count in [None,Some("0"),Some("12"),Some("13"),Some("013"),Some("14"),Some("24"),Some("64")] {
                let expected=matches!((mode,count),(None,Some("12"))|(Some("YES_QEMU_VM"),Some("13")));
                assert_eq!(direct_gpu_frame_budget_matches(mode,count),expected);
            }
        }
    }
    #[test]
    fn drag_budget_requires_exact_retained_opt_in() {
        assert!(direct_gpu_move_budget_matches(Some("YES_QEMU_VM"),Some("VISIBILITY_QEMU_VM"),Some("17")));
        assert!(!direct_gpu_move_budget_matches(None,Some("VISIBILITY_QEMU_VM"),Some("17")));
        assert!(!direct_gpu_move_budget_matches(Some("YES_QEMU_VM"),Some("VISIBILITY_QEMU_VM"),Some("15")));
        assert!(!direct_gpu_move_budget_matches(Some("YES_QEMU_VM"),Some("LIVE_QEMU_VM"),Some("17")));
        assert!(direct_gpu_move_budget_matches(Some("YES_QEMU_VM"),Some("LIVE_QEMU_VM"),Some("15")));
        assert!(!direct_gpu_move_budget_matches(None,Some("LIVE_QEMU_VM"),Some("15")));
        assert!(!direct_gpu_move_budget_matches(Some("YES_QEMU_VM"),Some("LIVE_QEMU_VM"),Some("14")));
        assert!(!direct_gpu_move_budget_matches(Some("YES_QEMU_VM"),Some("YES_QEMU_VM"),Some("15")));
        assert!(!direct_gpu_move_budget_matches(Some("YES_QEMU_VM"),None,Some("15")));
        assert!(direct_gpu_move_budget_matches(Some("YES_QEMU_VM"),Some("YES_QEMU_VM"),Some("14")));
        assert!(!direct_gpu_move_budget_matches(None,Some("YES_QEMU_VM"),Some("14")));
        assert!(!direct_gpu_move_budget_matches(Some("YES_QEMU_VM"),None,Some("14")));
        assert!(!direct_gpu_move_budget_matches(Some("YES_QEMU_VM"),Some("NO"),Some("14")));
        assert!(!direct_gpu_move_budget_matches(Some("YES_QEMU_VM"),Some("YES_QEMU_VM"),Some("13")));
        assert!(direct_gpu_move_budget_matches(Some("YES_QEMU_VM"),None,Some("13")));
    }
    #[test]
    fn old_twelve_frame_proof_unchanged() {
        assert_eq!(frame_budget(None,DEFAULT_QEMU_FRAMES),Ok(12));
        assert_eq!(frame_budget(Some("12"),12),Ok(12));
    }

    #[test]
    fn longer_scenarios_within_receiver_capacity() {
        for count in [13u64,20,24,32,48,64] {
            assert_eq!(frame_budget(Some(&count.to_string()),12),Ok(count));
        }
        assert_eq!(frame_budget(Some("65"),12),Err(FrameBudgetError::InvalidValue));
        assert_eq!(frame_budget(Some("1000000000"),12),Err(FrameBudgetError::InvalidValue));
    }

    #[test]
    fn receiver_rejects_mismatch_and_invalid_opt_in() {
        assert_eq!(receiver_frame_budget(None, None), Ok(12));
        for count in [8u64, 11, 12, 24, 64] {
            assert_eq!(receiver_frame_budget(Some(&count.to_string()), None), Ok(count));
        }
        for invalid in ["", "-1", "0", "7", "65", "NaN", "24x", " 24", "24 ",
                        "18446744073709551616"] {
            assert_eq!(receiver_frame_budget(Some(invalid), None),
                       Err(FrameBudgetError::InvalidValue));
        }
        assert_eq!(receiver_frame_budget(None, Some(1)),
                   Err(FrameBudgetError::InvalidValue));
        assert_eq!(receiver_frame_budget(Some("1"), None),
                   Err(FrameBudgetError::InvalidValue));
        assert_eq!(receiver_frame_budget(Some("1"), Some(1)), Ok(1));
        assert_eq!(receiver_frame_budget(Some("2"), Some(2)), Ok(2));
        assert_eq!(receiver_frame_budget(Some("3"), Some(3)), Ok(3));
        assert_eq!(receiver_frame_budget(Some("12"), Some(1)),
                   Err(FrameBudgetError::InvalidValue));
        assert_eq!(receiver_frame_budget(Some("2"), Some(3)),
                   Err(FrameBudgetError::InvalidValue));
    }

    #[test]
    fn invalid_or_injected_values_never_fall_back_silently() {
        for bad in ["", "0", "8", "-1", "12x", " 24", "24 ", "1e2", "nan"] {
            assert_eq!(frame_budget(Some(bad),12),Err(FrameBudgetError::InvalidValue));
        }
        assert_eq!(frame_budget(None,11),Err(FrameBudgetError::InvalidDefault));
        assert_eq!(frame_budget(None,65),Err(FrameBudgetError::InvalidDefault));
    }
}

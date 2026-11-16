//! Permanent fatal submission latch for an owned native KMS session.
//! Smithay input/commit handlers often return (), so a failed transport
//! submission must cross that boundary without being swallowed by logging.
//! A new DRM ownership epoch needs a new session; never reset this latch.
#[derive(Debug,Default)]
pub struct SubmissionFailure(Option<String>);
impl SubmissionFailure {
    pub fn record(&mut self,error:String) {
        if self.0.is_none() {self.0=Some(error);}
    }
    pub fn check(&self)->Result<(),String> {
        match self.0.as_ref() {
            None=>Ok(()),
            Some(error)=>Err(format!("native KMS submission failed permanently: {error}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn initial_session_allows_frames() {
        assert!(SubmissionFailure::default().check().is_ok());
    }
    #[test]
    fn first_submission_error_is_permanent_and_diagnostic() {
        let mut failure=SubmissionFailure::default();
        failure.record("GPU transport disconnected".into());
        assert!(failure.check().unwrap_err().contains("GPU transport disconnected"));
        failure.record("later unrelated error".into());
        let first=failure.check().unwrap_err();
        assert!(first.contains("GPU transport disconnected"));
        assert!(!first.contains("later unrelated error"));
        assert_eq!(failure.check().unwrap_err(),first);
    }
}

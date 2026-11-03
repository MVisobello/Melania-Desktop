//! Bounded 120ths-of-an-integer output scaling, CPU-side only.
//! Wayland fractional-scale negotiation is NOT enabled by this type.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct OutputScale120(u32);
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ScaleError {InvalidScale,InvalidCoordinate}

impl OutputScale120 {
    /// Support logical scales from 1x through 4x in 1/120 increments.
    pub fn new(units:u32)->Result<Self,ScaleError> {
        if !(120..=480).contains(&units) {return Err(ScaleError::InvalidScale);}
        Ok(Self(units))
    }
    pub fn numerator(self)->u32 {self.0}
    pub fn physical_to_logical(self,value:f64)->Option<f64> {
        value.is_finite().then_some(value*120.0/f64::from(self.0))
    }
    pub fn logical_to_physical(self,value:f64)->Result<i32,ScaleError> {
        if !value.is_finite() {return Err(ScaleError::InvalidCoordinate);}
        let scaled=(value*f64::from(self.0)/120.0).round();
        if scaled<f64::from(i32::MIN) || scaled>f64::from(i32::MAX) {
            return Err(ScaleError::InvalidCoordinate);
        }
        Ok(scaled as i32)
    }
    pub fn physical_extent(self,length:u32)->Option<u32> {
        let pixels=u64::from(length).checked_mul(u64::from(self.0))?;
        u32::try_from((pixels+60)/120).ok()
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn stable_one_and_fractional_output_scales() {
        let one=OutputScale120::new(120).unwrap();
        assert_eq!(one.logical_to_physical(400.0),Ok(400));
        assert_eq!(one.physical_extent(800),Some(800));
        let one_half=OutputScale120::new(180).unwrap();
        assert_eq!(one_half.logical_to_physical(200.0),Ok(300));
        assert_eq!(one_half.physical_to_logical(300.0),Some(200.0));
        assert_eq!(one_half.physical_extent(3),Some(5));
    }
    #[test]
    fn rejects_unbounded_and_nonfinite_geometry() {
        assert_eq!(OutputScale120::new(0),Err(ScaleError::InvalidScale));
        assert_eq!(OutputScale120::new(119),Err(ScaleError::InvalidScale));
        assert_eq!(OutputScale120::new(481),Err(ScaleError::InvalidScale));
        let s=OutputScale120::new(240).unwrap();
        assert_eq!(s.logical_to_physical(f64::NAN),Err(ScaleError::InvalidCoordinate));
        assert_eq!(s.logical_to_physical(1e300),Err(ScaleError::InvalidCoordinate));
        assert_eq!(s.physical_to_logical(f64::INFINITY),None);
    }
}

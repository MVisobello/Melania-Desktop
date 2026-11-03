//! Auditable prerequisites for an eventual native Wayland login session.
//! Evidence is supplied by real subsystem probes, never inferred from
//! successful QEMU pixel tests. The final production authorization is OFF.
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub struct SessionPrerequisites {
    pub libseat_active:bool,
    pub drm_master_owned:bool,
    pub output_confirmed:bool,
    pub vt_restore_ready:bool,
    pub input_enumerated:bool,
    pub xdg_shell_ready:bool,
    pub session_bus_ready:bool,
    pub polkit_ready:bool,
    pub lock_surface_ready:bool,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum MissingCapability {
    Libseat,DrmMaster,Output,VtRestore,Input,Shell,SessionBus,Polkit,SecureLock,
}
impl SessionPrerequisites {
    pub fn blockers(&self)->Vec<MissingCapability> {
        use MissingCapability::*;
        let mut missing=Vec::new();
        if !self.libseat_active {missing.push(Libseat);}
        if !self.drm_master_owned {missing.push(DrmMaster);}
        if !self.output_confirmed {missing.push(Output);}
        if !self.vt_restore_ready {missing.push(VtRestore);}
        if !self.input_enumerated {missing.push(Input);}
        if !self.xdg_shell_ready {missing.push(Shell);}
        if !self.session_bus_ready {missing.push(SessionBus);}
        if !self.polkit_ready {missing.push(Polkit);}
        if !self.lock_surface_ready {missing.push(SecureLock);}
        missing
    }
    pub fn evidence_complete(&self)->bool {self.blockers().is_empty()}
    /// Deliberate hard gate: production startup wiring and hardware
    /// verification have not been implemented by this model.
    pub fn native_login_authorized(&self)->bool {false}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_probe_means_no_claimed_native_login() {
        let p=SessionPrerequisites::default();
        assert_eq!(p.blockers().len(),9);
        assert!(!p.evidence_complete());
        assert!(!p.native_login_authorized());
    }
    #[test]
    fn even_all_passing_flags_cannot_enable_an_unimplemented_login() {
        let p=SessionPrerequisites {
            libseat_active:true,drm_master_owned:true,output_confirmed:true,
            vt_restore_ready:true,input_enumerated:true,xdg_shell_ready:true,
            session_bus_ready:true,polkit_ready:true,lock_surface_ready:true,
        };
        assert!(p.evidence_complete());
        assert_eq!(p.blockers(),vec![]);
        assert!(!p.native_login_authorized());
    }
}

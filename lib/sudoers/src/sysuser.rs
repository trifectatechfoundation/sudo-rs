/// This trait/module is here to not make this crate dependent (at the present time) in the idiosyncracies of user representation details
/// (which we may decide over time), as well as to make explicit what functionality a user-representation must have; this
/// interface is not set in stone and "easy" to change.
pub trait Identifiable: Eq {
    fn has_name(&self, _name: &str) -> bool {
        false
    }
    fn has_id(&self, _uid: u16) -> bool {
        false
    }

    fn is_root(&self) -> bool {
        false
    }
    fn in_group_by_name(&self, _name: &str) -> bool {
        false
    }
    fn in_group_by_gid(&self, _name: u16) -> bool {
        false
    }
}

impl Identifiable for &str {
    fn has_name(&self, name: &str) -> bool {
        *self == name
    }

    fn in_group_by_name(&self, name: &str) -> bool {
        self.has_name(name)
    }

    fn is_root(&self) -> bool {
        self.has_name("root")
    }
}

impl Identifiable for u16 {
    fn has_id(&self, uid: u16) -> bool {
        *self == uid
    }

    fn is_root(&self) -> bool {
        self.has_id(0)
    }
}

impl Identifiable for (&str, u16) {
    fn has_name(&self, name: &str) -> bool {
        self.0 == name
    }

    fn is_root(&self) -> bool {
        *self == ("root", 0)
    }
}

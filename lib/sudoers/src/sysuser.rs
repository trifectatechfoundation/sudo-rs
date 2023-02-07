/// This trait/module is here to not make this crate dependent (at the present time) in the idiosyncracies of user representation details
/// (which we may decide over time), as well as to make explicit what functionality a user-representation must have; this
/// interface is not set in stone and "easy" to change.
pub trait Identifiable {
    fn has_name(&self, _name: &str) -> bool {
        false
    }
    fn has_uid(&self, _uid: u16) -> bool {
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

pub trait UnixGroup {
    fn as_gid(&self) -> u16;
    fn try_as_name(&self) -> Option<&str>;
}

/// This is the "canonical" info that we need
#[derive(Debug)]
pub struct GroupID(pub u16, pub Option<String>);
#[derive(Debug)]
pub struct UserRecord(pub u16, pub Option<String>, pub Vec<GroupID>);

impl PartialEq<UserRecord> for UserRecord {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

impl Eq for UserRecord {}

impl UnixGroup for GroupID {
    fn as_gid(&self) -> u16 {
        self.0
    }

    fn try_as_name(&self) -> Option<&str> {
        self.1.as_deref()
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

impl Identifiable for GroupID {
    fn has_uid(&self, uid: u16) -> bool {
        self.0 == uid
    }
    fn has_name(&self, name: &str) -> bool {
        self.1.as_ref().map_or(false, |s| s == name)
    }
}

impl Identifiable for UserRecord {
    fn is_root(&self) -> bool {
        self.has_name("root") && self.has_uid(0)
    }

    fn in_group_by_name(&self, name: &str) -> bool {
        self.2.iter().any(|g| g.has_name(name))
    }

    fn in_group_by_gid(&self, id: u16) -> bool {
        self.2.iter().any(|g| g.has_uid(id))
    }
}

impl Identifiable for sudo_system::User {
    fn has_name(&self, name: &str) -> bool {
        self.name == name
    }
    fn has_uid(&self, uid: u16) -> bool {
        self.uid as u16 == uid
    }

    fn is_root(&self) -> bool {
        self.has_uid(0)
    }
    fn in_group_by_name(&self, _name: &str) -> bool {
        false
    }
    fn in_group_by_gid(&self, _name: u16) -> bool {
        false
    }
}

impl UnixGroup for sudo_system::Group {
    fn as_gid(&self) -> u16 {
        self.gid as u16
    }

    fn try_as_name(&self) -> Option<&str> {
        Some(&self.name)
    }
}

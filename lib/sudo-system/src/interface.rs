use super::Group;

pub type GroupId = libc::gid_t;
pub type UserId = libc::uid_t;

/// This trait/module is here to not make this crate independent (at the present time) in the idiosyncracies of user representation details
/// (which we may decide over time), as well as to make explicit what functionality a user-representation must have; this
/// interface is not set in stone and "easy" to change.
pub trait UnixUser {
    fn has_name(&self, _name: &str) -> bool {
        false
    }
    fn has_uid(&self, _uid: libc::uid_t) -> bool {
        false
    }
    fn is_root(&self) -> bool {
        false
    }
    fn in_group_by_name(&self, _name: &str) -> bool {
        false
    }
    fn in_group_by_gid(&self, _gid: GroupId) -> bool {
        false
    }
}

pub trait UnixGroup {
    fn as_gid(&self) -> GroupId;
    fn try_as_name(&self) -> Option<&str>;
}

impl UnixUser for &str {
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

impl UnixUser for super::User {
    fn has_name(&self, name: &str) -> bool {
        self.name == name
    }
    fn has_uid(&self, uid: GroupId) -> bool {
        self.uid == uid
    }
    fn is_root(&self) -> bool {
        self.has_uid(0)
    }
    fn in_group_by_name(&self, name: &str) -> bool {
        if let Ok(Some(group)) = Group::from_name(name) {
            self.in_group_by_gid(group.gid)
        } else {
            false
        }
    }
    fn in_group_by_gid(&self, gid: GroupId) -> bool {
        match &self.groups {
            Some(ids) => ids.contains(&gid),
            _ => false,
        }
    }
}

impl UnixGroup for super::Group {
    fn as_gid(&self) -> GroupId {
        self.gid
    }

    fn try_as_name(&self) -> Option<&str> {
        Some(&self.name)
    }
}

impl UnixGroup for (u16, &str) {
    fn try_as_name(&self) -> Option<&str> {
        Some(self.1)
    }
    fn as_gid(&self) -> GroupId {
        self.0 as GroupId
    }
}

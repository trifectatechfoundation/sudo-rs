use std::{ffi::CStr, fmt::Display, num::ParseIntError, str::FromStr};

/// Represents a group ID in the system.
///
/// `GroupId` is transparent because the memory mapping should stay the same as the underlying
/// type, so we can safely cast as a pointer.
/// See the implementation in `system::mod::set_target_user`.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GroupId(libc::gid_t);

/// Represents a user ID in the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UserId(libc::uid_t);

/// Represents a process ID in the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId(libc::pid_t);

/// Represents a device ID in the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(libc::dev_t);

impl GroupId {
    pub fn new(id: libc::gid_t) -> Self {
        Self(id)
    }

    pub fn inner(&self) -> libc::gid_t {
        self.0
    }
}

impl UserId {
    pub fn new(id: libc::uid_t) -> Self {
        Self(id)
    }

    pub fn inner(&self) -> libc::uid_t {
        self.0
    }

    pub const ROOT: Self = Self(0);
}

impl ProcessId {
    pub fn new(id: libc::pid_t) -> Self {
        Self(id)
    }

    pub fn inner(&self) -> libc::pid_t {
        self.0
    }

    pub fn is_valid(&self) -> bool {
        self.0 != 0
    }
}

impl DeviceId {
    pub fn new(id: libc::dev_t) -> Self {
        Self(id)
    }

    pub fn inner(&self) -> libc::dev_t {
        self.0
    }
}

impl Display for GroupId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Display for ProcessId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for GroupId {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<libc::gid_t>().map(GroupId::new)
    }
}

impl FromStr for UserId {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<libc::uid_t>().map(UserId::new)
    }
}

/// This trait/module is here to not make this crate independent (at the present time) in the idiosyncrasies of user representation details
/// (which we may decide over time), as well as to make explicit what functionality a user-representation must have; this
/// interface is not set in stone and "easy" to change.
pub trait UnixUser {
    fn has_name(&self, _name: &str) -> bool;
    fn has_uid(&self, _uid: UserId) -> bool;
    fn is_root(&self) -> bool;
    fn in_group_by_name(&self, _name: &CStr) -> bool;
    fn in_group_by_gid(&self, _gid: GroupId) -> bool;

    type Group: UnixGroup;
    fn group(&self) -> Self::Group;
}

pub trait UnixGroup {
    fn as_gid(&self) -> GroupId;
    fn try_as_name(&self) -> Option<&str>;
}

impl UnixUser for super::User {
    fn has_name(&self, name: &str) -> bool {
        self.name == name
    }
    fn has_uid(&self, uid: UserId) -> bool {
        self.uid == uid
    }
    fn is_root(&self) -> bool {
        self.has_uid(UserId::ROOT)
    }
    fn in_group_by_name(&self, name_c: &CStr) -> bool {
        if let Ok(Some(group)) = super::Group::from_name(name_c) {
            self.in_group_by_gid(group.gid)
        } else {
            false
        }
    }
    fn in_group_by_gid(&self, gid: GroupId) -> bool {
        self.groups.contains(&gid)
    }
    type Group = super::Group;
    fn group(&self) -> super::Group {
        Self::Group {
            gid: self.gid,
            name: None,
        }
    }
}

impl UnixGroup for super::Group {
    fn as_gid(&self) -> GroupId {
        self.gid
    }
    fn try_as_name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::system::{Group, User, ROOT_GROUP_NAME};
    use std::ffi::CString;

    fn test_user(user: impl UnixUser, name_c: &CStr) {
        let name = name_c.to_str().unwrap();
        assert!(user.has_name(name));
        if user.has_name("root") {
            assert!(user.in_group_by_name(CString::new(ROOT_GROUP_NAME).unwrap().as_c_str()));
            assert!(user.is_root());
        } else {
            assert!(user.in_group_by_name(name_c));
            assert!(!user.is_root());
        }
        assert_eq!(user.is_root(), name == "root");
    }

    fn test_group(group: impl UnixGroup, name: &str) {
        assert_eq!(name == ROOT_GROUP_NAME, group.as_gid() == GroupId::new(0));
        assert_eq!(group.try_as_name(), Some(name));
    }

    #[test]
    fn test_unix_user() {
        let user = |name| User::from_name(name).unwrap().unwrap();
        test_user(user(c"root"), c"root");
        test_user(user(c"daemon"), c"daemon");
    }

    #[test]
    fn test_unix_group() {
        let group = |name| Group::from_name(name).unwrap().unwrap();
        let root_group_cstr = CString::new(ROOT_GROUP_NAME).unwrap();
        test_group(group(root_group_cstr.as_c_str()), ROOT_GROUP_NAME);
        test_group(group(c"daemon"), "daemon");
    }
}

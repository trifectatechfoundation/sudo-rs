use std::{ffi::CStr, fmt::Display, num::ParseIntError, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GroupId(libc::gid_t);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UserId(libc::uid_t);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId(libc::pid_t);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(libc::dev_t);

impl GroupId {
    pub fn new(id: libc::gid_t) -> Self {
        Self(id)
    }

    pub fn get(&self) -> libc::gid_t {
        self.0
    }
}

impl UserId {
    pub fn new(id: libc::uid_t) -> Self {
        Self(id)
    }

    pub fn get(&self) -> libc::uid_t {
        self.0
    }

    #[cfg(test)]
    pub const fn new_const(uid: libc::uid_t) -> Self {
        UserId(uid)
    }
}

impl ProcessId {
    pub fn new(id: libc::pid_t) -> Self {
        Self(id)
    }

    pub fn get(&self) -> libc::pid_t {
        self.0
    }
}

impl DeviceId {
    pub fn new(id: libc::dev_t) -> Self {
        Self(id)
    }

    pub fn get(&self) -> libc::dev_t {
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

pub trait UnixUser {
    fn has_name(&self, _name: &str) -> bool {
        false
    }
    fn has_uid(&self, _uid: UserId) -> bool {
        false
    }
    fn is_root(&self) -> bool {
        false
    }
    fn in_group_by_name(&self, _name: &CStr) -> bool {
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

impl UnixUser for super::User {
    fn has_name(&self, name: &str) -> bool {
        self.name == name
    }
    fn has_uid(&self, uid: UserId) -> bool {
        self.uid == uid
    }
    fn is_root(&self) -> bool {
        self.has_uid(UserId::new(0))
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
}

impl UnixGroup for super::Group {
    fn as_gid(&self) -> GroupId {
        self.gid
    }
    fn try_as_name(&self) -> Option<&str> {
        Some(&self.name)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::system::{Group, User};

    fn test_user(user: impl UnixUser, name_c: &CStr, uid: UserId) {
        let name = name_c.to_str().unwrap();
        assert!(user.has_name(name));
        assert!(user.has_uid(uid));
        assert!(user.in_group_by_name(name_c));
        assert_eq!(user.is_root(), name == "root");
    }

    fn test_group(group: impl UnixGroup, name: &str, gid: GroupId) {
        assert_eq!(group.as_gid(), gid);
        assert_eq!(group.try_as_name(), Some(name));
    }

    #[test]
    fn test_unix_user() {
        let user = |name| User::from_name(name).unwrap().unwrap();
        test_user(user(cstr!("root")), cstr!("root"), UserId::new(0));
        test_user(user(cstr!("daemon")), cstr!("daemon"), UserId::new(1));
    }

    #[test]
    fn test_unix_group() {
        let group = |name| Group::from_name(name).unwrap().unwrap();
        test_group(group(cstr!("root")), "root", GroupId::new(0));
        test_group(group(cstr!("daemon")), "daemon", GroupId::new(1));
    }

    #[test]
    fn test_default() {
        impl UnixUser for () {}
        assert!(!().has_name("root"));
        assert!(!().has_uid(UserId::new(0)));
        assert!(!().is_root());
        assert!(!().in_group_by_name(cstr!("root")));
    }
}
